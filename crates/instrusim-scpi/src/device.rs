//! El contrato mínimo de un dispositivo SCPI y el despacho de lo obligatorio.
//!
//! IEEE 488.2 exige que **todos** los dispositivos entiendan un puñado de
//! comandos comunes: `*IDN?`, `*RST`, `*CLS`, `*ESE`, `*ESR?`, `*OPC`, `*SRE`,
//! `*STB?`, `*TST?` y `*WAI`. SCPI añade `SYSTem:ERRor?` y `SYSTem:VERSion?`.
//!
//! Están implementados una sola vez, aquí, y valen tanto para un instrumento
//! escrito en Rust como para uno descrito en un perfil YAML. Antes vivían en
//! `instrusim-model`, lo que ataba el despacho al motor de simulación y obligó
//! al linaje declarativo de Crucible a escribir su propio SCPI paralelo. Esa
//! duplicación es justo lo que este módulo elimina.
//!
//! El contrato **no conoce el mundo simulado**. Un instrumento que necesite
//! leer sus terminales captura la referencia al `World` antes de despachar
//! (ver el adaptador de `instrusim-model`); uno declarativo no necesita nada.

use crate::error::{ErrorCode, ErrorQueue, ScpiError};
use crate::format;
use crate::status::{StatusModel, esr};
use crate::{Command, CommandTable, parse_message};
use std::sync::OnceLock;

/// Lo que hace falta para hablar SCPI, sin suponer cómo está implementado.
pub trait ScpiDevice {
    /// La cadena que contesta `*IDN?`: cuatro campos separados por comas.
    fn idn(&self) -> String;

    /// Registros de estado de IEEE 488.2. El despacho común los manipula.
    fn status(&mut self) -> &mut StatusModel;

    /// Cola de errores SCPI.
    fn errors(&mut self) -> &mut ErrorQueue;

    /// Vuelve al estado de encendido. Es lo que hace `*RST`.
    fn reset(&mut self);

    /// Autotest. Cero significa que ha ido bien, que es lo que exige `*TST?`.
    fn self_test(&mut self) -> i64 {
        0
    }

    /// Los comandos propios de este dispositivo.
    ///
    /// No hace falta tratar aquí los comunes ni `SYSTem:ERRor?`: cuando esta
    /// función se llama, ya se ha descartado que el comando fuese de esos.
    fn execute(&mut self, cmd: &Command) -> Result<Option<String>, ScpiError>;
}

/// Procesa una línea recibida y devuelve la línea a contestar, si procede.
///
/// Un mensaje puede contener varias consultas; sus respuestas se devuelven
/// unidas por punto y coma, como manda el estándar. Si no hubo ninguna
/// consulta, no se contesta nada.
///
/// Los errores no interrumpen la conversación: se anotan en la cola, se enciende
/// el bit correspondiente del registro de sucesos y se sigue. Es lo que hace un
/// instrumento real, y es justo lo que permite al cliente descubrir después qué
/// salió mal con `SYSTem:ERRor?`.
pub fn handle_message(device: &mut dyn ScpiDevice, line: &str) -> Option<String> {
    let comandos = match parse_message(line) {
        Ok(c) => c,
        Err(e) => {
            anotar(device, e);
            return None;
        }
    };

    let mut respuestas: Vec<String> = Vec::new();

    for cmd in &comandos {
        let resultado = if cmd.is_common() {
            comando_comun(device, cmd)
        } else if let Some(r) = comando_de_sistema(device, cmd) {
            r
        } else {
            device.execute(cmd)
        };

        match resultado {
            Ok(Some(r)) => respuestas.push(r),
            Ok(None) => {}
            Err(e) => {
                anotar(device, e);
                // El estándar manda abandonar el resto del mensaje en cuanto
                // uno de sus comandos falla: seguir ejecutando lo que venía
                // detrás de una configuración fallida produciría un estado
                // incoherente que el cliente no espera.
                break;
            }
        }
    }

    if respuestas.is_empty() {
        None
    } else {
        Some(respuestas.join(";"))
    }
}

/// Anota el error en la cola y enciende el bit de suceso de su familia.
fn anotar(device: &mut dyn ScpiDevice, error: ScpiError) {
    let bit = error.code.esr_bit();
    device.errors().push(error);
    device.status().set_event(bit);
}

/// Los comandos comunes de IEEE 488.2, los que empiezan por asterisco.
fn comando_comun(device: &mut dyn ScpiDevice, cmd: &Command) -> Result<Option<String>, ScpiError> {
    match (cmd.header.as_str(), cmd.query) {
        ("*IDN", true) => Ok(Some(device.idn())),

        ("*RST", false) => {
            device.reset();
            Ok(None)
        }

        ("*CLS", false) => {
            device.errors().clear();
            device.status().clear();
            Ok(None)
        }

        ("*ESE", false) => {
            let v = cmd.numeric(0)?.resolve(0.0, 255.0, 0.0);
            device.status().set_event_enable(mascara(v)?);
            Ok(None)
        }
        ("*ESE", true) => {
            let v = device.status().event_enable();
            Ok(Some(format::nr1(v as i64)))
        }

        ("*ESR", true) => {
            let v = device.status().read_event();
            Ok(Some(format::nr1(v as i64)))
        }

        ("*SRE", false) => {
            let v = cmd.numeric(0)?.resolve(0.0, 255.0, 0.0);
            device.status().set_service_enable(mascara(v)?);
            Ok(None)
        }
        ("*SRE", true) => {
            let v = device.status().service_enable();
            Ok(Some(format::nr1(v as i64)))
        }

        ("*STB", true) => {
            let hay_errores = !device.errors().is_empty();
            let v = device.status().status_byte(hay_errores, false);
            Ok(Some(format::nr1(v as i64)))
        }

        // Sin comandos solapados, toda operación termina en el acto, así que
        // `*OPC` puede encender el bit de inmediato y `*OPC?` contestar ya.
        ("*OPC", false) => {
            device.status().set_event(esr::OPC);
            Ok(None)
        }
        ("*OPC", true) => Ok(Some("1".to_string())),

        ("*TST", true) => {
            let r = device.self_test();
            Ok(Some(format::nr1(r)))
        }

        ("*WAI", false) => Ok(None),

        _ => Err(ScpiError::with_detail(
            ErrorCode::UndefinedHeader,
            &cmd.header,
        )),
    }
}

/// Comandos del subsistema `SYSTem` que son iguales en todos los dispositivos.
///
/// Devuelve `None` si la cabecera no es de este subsistema, para que el
/// dispositivo la trate como suya.
fn comando_de_sistema(
    device: &mut dyn ScpiDevice,
    cmd: &Command,
) -> Option<Result<Option<String>, ScpiError>> {
    #[derive(Clone, Copy)]
    enum Sys {
        Error,
        Version,
    }

    // La tabla se compila una sola vez en toda la vida del proceso. `OnceLock`
    // es la inicialización perezosa y segura entre hilos de la librería
    // estándar: el primer hilo que llegue la construye y el resto reutilizan.
    static TABLA: OnceLock<CommandTable<Sys>> = OnceLock::new();
    let tabla = TABLA.get_or_init(|| {
        CommandTable::from_pairs([
            ("SYSTem:ERRor[:NEXT]", Sys::Error),
            ("SYSTem:VERSion", Sys::Version),
        ])
    });

    let (accion, _) = tabla.lookup(&cmd.header)?;

    if !cmd.query {
        // Ambas son solo de consulta; usarlas como orden es error de comando.
        return Some(Err(ScpiError::with_detail(
            ErrorCode::UndefinedHeader,
            &cmd.header,
        )));
    }

    Some(match accion {
        Sys::Error => Ok(Some(device.errors().pop().to_string())),
        // Versión de SCPI a la que se ajusta el dispositivo.
        Sys::Version => Ok(Some("1999.0".to_string())),
    })
}

/// Convierte un parámetro numérico en una máscara de ocho bits.
fn mascara(v: f64) -> Result<u8, ScpiError> {
    if !(0.0..=255.0).contains(&v) {
        return Err(ScpiError::new(ErrorCode::DataOutOfRange));
    }
    Ok(v as u8)
}
