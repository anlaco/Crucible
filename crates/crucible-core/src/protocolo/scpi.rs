//! SCPI declarativo: un dispositivo descrito en YAML que habla el protocolo de
//! verdad.
//!
//! Este módulo **no implementa SCPI**. El protocolo —abreviaturas, nodos
//! opcionales, sufijos de canal, mensajes compuestos, cola de errores, registros
//! de IEEE 488.2— vive una sola vez en `instrusim-scpi`. Aquí solo se traduce el
//! perfil a lo que aquel motor espera: una [`CommandTable`] y un `execute`.
//!
//! Antes había aquí un segundo SCPI, escrito a mano, que comparaba la línea
//! recibida con el patrón carácter a carácter. Funcionaba con los patrones que
//! él mismo escribía y fallaba con todo lo demás: `SOURce:VOLTage` no casaba con
//! `sour:volt`, un `*IDN?;*RST` compuesto se perdía entero, y un comando
//! desconocido cerraba la conversación en vez de anotarse en la cola. Ver
//! ADR-0003.

use crate::error::{CrucibleError, Result};
use crate::estado::Estado;
use crate::modelo::EvaluadorModelos;
use crate::perfil::{Comando, Perfil};
use instrusim_scpi::error::{ErrorCode, ErrorQueue, ScpiError};
use instrusim_scpi::status::StatusModel;
use instrusim_scpi::{Command, CommandTable, ScpiDevice, handle_message};
use std::collections::HashMap;

/// Un dispositivo cuyo comportamiento viene de un perfil, no de código Rust.
///
/// Cumple el mismo contrato [`ScpiDevice`] que los instrumentos escritos a mano
/// de `instrusim-model`, así que hereda gratis todo lo obligatorio: `*IDN?`,
/// `*RST`, `*CLS`, `SYSTem:ERRor?`, el registro de sucesos y el byte de estado.
pub struct DispositivoScpi {
    perfil: Perfil,
    pub estado: Estado,
    /// Patrones compilados una sola vez, no en cada mensaje. El `usize` indexa
    /// `perfil.comandos`.
    ///
    /// Van en **dos tablas** porque el mismo patrón suele declararse dos veces
    /// —`SOUR:VOLT 5` y `SOUR:VOLT?` son cosas distintas— y `lookup` devuelve
    /// la primera coincidencia: con una sola tabla, la consulta quedaría
    /// siempre tapada por la orden.
    ordenes: CommandTable<usize>,
    consultas: CommandTable<usize>,
    status: StatusModel,
    errores: ErrorQueue,
    evaluador: EvaluadorModelos,
}

impl DispositivoScpi {
    pub fn nuevo(perfil: Perfil) -> Self {
        let estado = Estado::from_hashmap(&perfil.estado);
        let ordenes = compilar_tabla(&perfil.comandos, false);
        let consultas = compilar_tabla(&perfil.comandos, true);
        Self {
            perfil,
            estado,
            ordenes,
            consultas,
            status: StatusModel::new(),
            errores: ErrorQueue::default(),
            // Semilla fija: dos ejecuciones del mismo banco dan los mismos
            // números. Es lo que permite meter un test de instrumentación en CI
            // y que no parpadee.
            evaluador: EvaluadorModelos::con_semilla(42),
        }
    }

    /// Procesa una línea y devuelve la respuesta, si la hay.
    pub fn procesar(&mut self, linea: &str) -> Option<String> {
        handle_message(self, linea)
    }

    pub fn perfil(&self) -> &Perfil {
        &self.perfil
    }
}

/// Compila los patrones del perfil que sean de la forma pedida (orden o
/// consulta). El índice es la posición en `comandos`, así que el orden de
/// declaración decide quién gana cuando dos patrones se solapan — lo específico
/// va antes que lo genérico, igual que en el motor.
fn compilar_tabla(comandos: &[Comando], query: bool) -> CommandTable<usize> {
    let mut tabla = CommandTable::new();
    for (i, cmd) in comandos.iter().enumerate() {
        if cmd.query == query {
            tabla.add(&cmd.patron, i);
        }
    }
    tabla
}

impl ScpiDevice for DispositivoScpi {
    fn idn(&self) -> String {
        self.perfil
            .dispositivo
            .idn
            .clone()
            .unwrap_or_else(|| format!("Crucible,{},0,1.0", self.perfil.dispositivo.modelo))
    }

    fn status(&mut self) -> &mut StatusModel {
        &mut self.status
    }

    fn errors(&mut self) -> &mut ErrorQueue {
        &mut self.errores
    }

    /// `*RST` devuelve el estado a los valores declarados en el perfil.
    fn reset(&mut self) {
        self.estado = Estado::from_hashmap(&self.perfil.estado);
    }

    fn execute(&mut self, cmd: &Command) -> Result0<Option<String>> {
        // La forma decide la tabla: una consulta nunca despierta la orden del
        // mismo nombre, ni al revés. Sin esto `SOUR:VOLT?` mutaría el estado.
        let tabla = if cmd.query {
            &self.consultas
        } else {
            &self.ordenes
        };

        let Some((&idx, _sufijos)) = tabla.lookup(&cmd.header) else {
            return Err(ScpiError::with_detail(
                ErrorCode::UndefinedHeader,
                &cmd.header,
            ));
        };

        let comando = &self.perfil.comandos[idx];

        // Los argumentos posicionales toman el nombre que declare el perfil;
        // si no declara ninguno, se referencian por posición: <0>, <1>.
        let args = nombrar_args(comando, &cmd.args);

        if let Some(muta) = &comando.muta {
            super::aplicar_mutacion(muta, &mut self.estado, &args);
        }

        if !cmd.query {
            return Ok(None);
        }

        if let Some(resp) = &comando.respuesta {
            return Ok(Some(super::resolver_plantilla(resp, &self.estado, &args)));
        }

        if let Some(nombre) = &comando.modelo {
            let modelo = self.perfil.modelos.get(nombre).ok_or_else(|| {
                // El perfil se valida al cargarlo, así que llegar aquí implica
                // que alguien lo mutó en caliente. Se trata como error del
                // dispositivo, no del cliente.
                ScpiError::with_detail(ErrorCode::ExecutionError, nombre)
            })?;
            let valor = self
                .evaluador
                .evaluar(modelo, &self.estado)
                .map_err(|e| ScpiError::with_detail(ErrorCode::ExecutionError, e.to_string()))?;
            return Ok(Some(valor));
        }

        // Consulta declarada sin respuesta ni modelo: el perfil está incompleto.
        Err(ScpiError::with_detail(
            ErrorCode::ExecutionError,
            &cmd.header,
        ))
    }
}

/// Alias local para no repetir el `Result` de SCPI en cada firma.
type Result0<T> = std::result::Result<T, ScpiError>;

/// Empareja los argumentos recibidos con los nombres que declara el perfil.
fn nombrar_args(comando: &Comando, recibidos: &[String]) -> HashMap<String, String> {
    let mut args = HashMap::new();
    for (i, valor) in recibidos.iter().enumerate() {
        args.insert(i.to_string(), valor.clone());
        if let Some(nombre) = comando.args.get(i) {
            args.insert(nombre.clone(), valor.clone());
        }
    }
    args
}

/// Construye un [`DispositivoScpi`] validando antes el perfil.
pub fn desde_perfil(perfil: Perfil) -> Result<DispositivoScpi> {
    if perfil.protocolo != crate::perfil::ProtocoloTipo::Scpi {
        return Err(CrucibleError::Protocolo(format!(
            "el perfil '{}' declara protocolo {:?}, no SCPI",
            perfil.dispositivo.modelo, perfil.protocolo
        )));
    }
    Ok(DispositivoScpi::nuevo(perfil))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perfil_keithley() -> Perfil {
        Perfil::from_yaml(
            r#"
dispositivo:
  modelo: KEITHLEY-2400
  idn: "Keithley,2400,1234567,A1.2"
protocolo: scpi
estado:
  voltaje_fuente: 0.0
  output: false
comandos:
  - patron: "OUTPut[:STATe]"
    args: [on]
    muta: { output: "<on>" }
  - patron: "SOURce:VOLTage[:LEVel]"
    args: [v]
    muta: { voltaje_fuente: "<v>" }
  - patron: "SOURce:VOLTage[:LEVel]"
    query: true
    respuesta: "{voltaje_fuente}"
  - patron: "MEASure:VOLTage[:DC]"
    query: true
    modelo: medir_voltaje
modelos:
  medir_voltaje:
    tipo: formula
    cuando: { output: "true" }
    expr: "voltaje_fuente"
    fallback: "0.0"
"#,
        )
        .expect("el perfil de prueba debe cargar")
    }

    fn dispositivo() -> DispositivoScpi {
        DispositivoScpi::nuevo(perfil_keithley())
    }

    #[test]
    fn idn_sale_del_perfil() {
        assert_eq!(
            dispositivo().procesar("*IDN?").as_deref(),
            Some("Keithley,2400,1234567,A1.2")
        );
    }

    /// Lo que el codec anterior no sabía hacer: la misma cabecera escrita de
    /// las formas que SCPI declara equivalentes.
    #[test]
    fn acepta_forma_corta_larga_y_minusculas() {
        for linea in [
            "SOUR:VOLT 5.0",
            "SOURCE:VOLTAGE 5.0",
            "sour:volt 5.0",
            "SOUR:VOLT:LEV 5.0",
        ] {
            let mut d = dispositivo();
            d.procesar(linea);
            assert_eq!(
                d.estado.get_float("voltaje_fuente"),
                Some(5.0),
                "no aceptó '{linea}'"
            );
        }
    }

    /// Los dos puntos iniciales devuelven la cabecera a la raíz. Sin ellos,
    /// SCPI la interpreta relativa al comando anterior (ver el test siguiente).
    #[test]
    fn un_mensaje_compuesto_ejecuta_todo_y_une_las_respuestas() {
        let mut d = dispositivo();
        let r = d.procesar("SOUR:VOLT 3.0;:OUTP ON;:MEAS:VOLT?");
        assert_eq!(r.as_deref(), Some("3.0"));
        assert_eq!(d.estado.get_bool("output"), Some(true));
    }

    /// La regla que sorprende a todo el mundo: en `SOUR:VOLT 1;OUTP ON`, el
    /// segundo comando es `SOUR:OUTP`, no `OUTP`. El perfil no declara esa
    /// cabecera, así que va a la cola de errores — que es exactamente lo que
    /// haría un instrumento real, y lo que el codec anterior no sabía ver.
    #[test]
    fn una_cabecera_encadenada_sin_dos_puntos_es_relativa() {
        let mut d = dispositivo();
        d.procesar("SOUR:VOLT 1.0;OUTP ON");
        assert_eq!(
            d.estado.get_bool("output"),
            Some(false),
            "'OUTP' tras 'SOUR:VOLT' debe leerse como 'SOUR:OUTP'"
        );
        let err = d.procesar("SYST:ERR?").unwrap();
        assert!(err.starts_with("-113"), "cola de errores devolvió '{err}'");
    }

    #[test]
    fn una_cabecera_desconocida_va_a_la_cola_de_errores() {
        let mut d = dispositivo();
        assert_eq!(d.procesar("FOO:BAR"), None);
        let err = d.procesar("SYST:ERR?").unwrap();
        assert!(err.starts_with("-113"), "cola de errores devolvió '{err}'");
    }

    /// El codec anterior cortaba la conversación al primer comando raro.
    #[test]
    fn el_dispositivo_sigue_hablando_despues_de_un_error() {
        let mut d = dispositivo();
        d.procesar("FOO:BAR");
        assert_eq!(
            d.procesar("*IDN?").as_deref(),
            Some("Keithley,2400,1234567,A1.2")
        );
    }

    #[test]
    fn una_consulta_no_muta_el_estado() {
        let mut d = dispositivo();
        d.procesar("SOUR:VOLT 7.0");
        d.procesar("SOUR:VOLT?");
        assert_eq!(d.estado.get_float("voltaje_fuente"), Some(7.0));
    }

    #[test]
    fn reset_devuelve_el_estado_al_del_perfil() {
        let mut d = dispositivo();
        d.procesar("SOUR:VOLT 9.0;OUTP ON");
        d.procesar("*RST");
        assert_eq!(d.estado.get_float("voltaje_fuente"), Some(0.0));
        assert_eq!(d.estado.get_bool("output"), Some(false));
    }

    #[test]
    fn el_modelo_respeta_su_condicion() {
        let mut d = dispositivo();
        d.procesar("SOUR:VOLT 4.0");
        // Con la salida apagada, el modelo cae al fallback.
        assert_eq!(d.procesar("MEAS:VOLT?").as_deref(), Some("0.0"));
        d.procesar("OUTP ON");
        assert_eq!(d.procesar("MEAS:VOLT?").as_deref(), Some("4.0"));
    }
}
