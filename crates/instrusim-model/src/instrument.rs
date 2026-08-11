//! El contrato que cumple todo instrumento simulado en Rust.
//!
//! IEEE 488.2 exige que **todos** los instrumentos entiendan un puñado de
//! comandos comunes: `*IDN?`, `*RST`, `*CLS`, `*ESE`, `*ESR?`, `*OPC`, `*SRE`,
//! `*STB?`, `*TST?` y `*WAI`. SCPI añade `SYSTem:ERRor?` y `SYSTem:VERSion?`.
//!
//! Están implementados una sola vez para todo el repositorio, en
//! `instrusim_scpi::device`, y valen igual para los instrumentos de este módulo
//! que para los descritos en un perfil YAML de `crucible-core`. Aquí solo queda
//! el contrato propio del motor —el que sí conoce el [`World`]— y el puente que
//! lo conecta con aquel despacho.
//!
//! Un instrumento concreto solo escribe lo suyo, y por construcción no puede
//! olvidarse de lo obligatorio ni implementarlo de forma distinta a sus
//! hermanos.

use instrusim_core::{Stepper, World};
use instrusim_scpi::error::{ErrorQueue, ScpiError};
use instrusim_scpi::status::StatusModel;
use instrusim_scpi::{Command, ScpiDevice, handle_message as scpi_handle_message};

/// La identidad que devuelve `*IDN?`.
///
/// El formato lo fija IEEE 488.2 y es rígido: cuatro campos separados por comas,
/// sin espacios alrededor. Muchos drivers deciden qué instrumento tienen delante
/// analizando exactamente esta cadena.
#[derive(Debug, Clone)]
pub struct Identity {
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub firmware: String,
}

impl Identity {
    pub fn new(
        manufacturer: impl Into<String>,
        model: impl Into<String>,
        serial: impl Into<String>,
        firmware: impl Into<String>,
    ) -> Self {
        Self {
            manufacturer: manufacturer.into(),
            model: model.into(),
            serial: serial.into(),
            firmware: firmware.into(),
        }
    }

    pub fn idn(&self) -> String {
        format!(
            "{},{},{},{}",
            self.manufacturer, self.model, self.serial, self.firmware
        )
    }
}

/// Un instrumento simulado.
///
/// Hereda de [`Stepper`] porque todo instrumento participa en la simulación:
/// aunque no haga nada en cada tic, la puerta está abierta para que modele
/// tiempos de establecimiento, deriva o disparos.
pub trait Instrument: Stepper {
    fn identity(&self) -> &Identity;

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

    /// Los comandos propios de este instrumento.
    ///
    /// No hace falta tratar aquí los comunes ni `SYSTem:ERRor?`: cuando esta
    /// función se llama, ya se ha descartado que el comando fuese de esos.
    fn execute(&mut self, cmd: &Command, world: &mut World) -> Result<Option<String>, ScpiError>;
}

/// Procesa una línea recibida y devuelve la línea a contestar, si procede.
///
/// El despacho de verdad —comandos comunes de IEEE 488.2, `SYSTem:ERRor?`,
/// mensajes compuestos, cola de errores— vive en `instrusim_scpi::device` y es
/// común a todo el repositorio. Aquí solo se tiende el puente: [`Instrument`]
/// necesita el [`World`] para resolver sus comandos y el contrato genérico
/// [`ScpiDevice`] no lo conoce, así que se empaquetan juntos durante el
/// despacho.
pub fn handle_message(
    instrument: &mut dyn Instrument,
    line: &str,
    world: &mut World,
) -> Option<String> {
    let mut puente = Puente { instrument, world };
    scpi_handle_message(&mut puente, line)
}

/// Une un instrumento con el mundo en el que vive, para que el par cumpla el
/// contrato [`ScpiDevice`], que es deliberadamente ajeno a la simulación.
struct Puente<'a> {
    instrument: &'a mut dyn Instrument,
    world: &'a mut World,
}

impl ScpiDevice for Puente<'_> {
    fn idn(&self) -> String {
        self.instrument.identity().idn()
    }

    fn status(&mut self) -> &mut StatusModel {
        self.instrument.status()
    }

    fn errors(&mut self) -> &mut ErrorQueue {
        self.instrument.errors()
    }

    fn reset(&mut self) {
        self.instrument.reset()
    }

    fn self_test(&mut self) -> i64 {
        self.instrument.self_test()
    }

    fn execute(&mut self, cmd: &Command) -> Result<Option<String>, ScpiError> {
        self.instrument.execute(cmd, self.world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use instrusim_scpi::error::ErrorCode;
    use instrusim_scpi::status::{esr, stb};
    use std::time::Duration;

    /// Instrumento mínimo para probar el despacho común sin depender de
    /// ninguno de los modelos reales.
    struct Maniqui {
        identity: Identity,
        status: StatusModel,
        errors: ErrorQueue,
        reseteado: bool,
    }

    impl Maniqui {
        fn new() -> Self {
            Self {
                identity: Identity::new("InstruSim", "MANIQUI", "0", "1.0"),
                status: StatusModel::new(),
                errors: ErrorQueue::default(),
                reseteado: false,
            }
        }
    }

    impl Stepper for Maniqui {
        fn step(&mut self, _world: &mut World, _dt: Duration) {}
    }

    impl Instrument for Maniqui {
        fn identity(&self) -> &Identity {
            &self.identity
        }
        fn status(&mut self) -> &mut StatusModel {
            &mut self.status
        }
        fn errors(&mut self) -> &mut ErrorQueue {
            &mut self.errors
        }
        fn reset(&mut self) {
            self.reseteado = true;
        }
        fn execute(
            &mut self,
            cmd: &Command,
            _world: &mut World,
        ) -> Result<Option<String>, ScpiError> {
            match (cmd.header.as_str(), cmd.query) {
                ("ECHO", true) => Ok(Some("eco".into())),
                _ => Err(ScpiError::with_detail(
                    ErrorCode::UndefinedHeader,
                    &cmd.header,
                )),
            }
        }
    }

    fn hablar(inst: &mut Maniqui, world: &mut World, line: &str) -> Option<String> {
        handle_message(inst, line, world)
    }

    #[test]
    fn idn_devuelve_los_cuatro_campos() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        assert_eq!(
            hablar(&mut i, &mut w, "*IDN?").as_deref(),
            Some("InstruSim,MANIQUI,0,1.0")
        );
    }

    #[test]
    fn un_comando_sin_consulta_no_contesta() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        assert_eq!(hablar(&mut i, &mut w, "*CLS"), None);
    }

    #[test]
    fn rst_llega_al_instrumento() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        hablar(&mut i, &mut w, "*RST");
        assert!(i.reseteado);
    }

    #[test]
    fn varias_consultas_en_un_mensaje_se_contestan_unidas_por_punto_y_coma() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        let r = hablar(&mut i, &mut w, "*IDN?;ECHO?").unwrap();
        assert_eq!(r, "InstruSim,MANIQUI,0,1.0;eco");
    }

    /// Un comando desconocido no rompe la conexión: se anota y el cliente lo
    /// descubre cuando pregunta.
    #[test]
    fn una_cabecera_desconocida_va_a_la_cola_de_errores() {
        let (mut i, mut w) = (Maniqui::new(), World::new());

        assert_eq!(hablar(&mut i, &mut w, "MEAS:PATATA?"), None);

        let e = hablar(&mut i, &mut w, "SYST:ERR?").unwrap();
        assert_eq!(e, "-113,\"Undefined header;MEAS:PATATA\"");

        // Y la cola queda limpia.
        let e = hablar(&mut i, &mut w, "SYST:ERR?").unwrap();
        assert_eq!(e, "0,\"No error\"");
    }

    #[test]
    fn un_error_enciende_su_bit_del_registro_de_sucesos() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        hablar(&mut i, &mut w, "*CLS"); // limpiar el bit de encendido

        hablar(&mut i, &mut w, "COMANDO:INVENTADO");

        let esr: u8 = hablar(&mut i, &mut w, "*ESR?").unwrap().parse().unwrap();
        assert_ne!(esr & (1 << esr::COMMAND_ERROR), 0);
    }

    #[test]
    fn el_resto_del_mensaje_se_abandona_tras_un_error() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        // La consulta de después no debe llegar a ejecutarse.
        assert_eq!(hablar(&mut i, &mut w, "NO:EXISTE;ECHO?"), None);
    }

    #[test]
    fn cls_vacia_la_cola_de_errores() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        hablar(&mut i, &mut w, "NO:EXISTE");
        hablar(&mut i, &mut w, "*CLS");

        assert_eq!(
            hablar(&mut i, &mut w, "SYST:ERR?").as_deref(),
            Some("0,\"No error\"")
        );
    }

    #[test]
    fn las_mascaras_de_estado_se_leen_y_escriben() {
        let (mut i, mut w) = (Maniqui::new(), World::new());

        hablar(&mut i, &mut w, "*ESE 32");
        assert_eq!(hablar(&mut i, &mut w, "*ESE?").as_deref(), Some("32"));

        hablar(&mut i, &mut w, "*SRE 32");
        assert_eq!(hablar(&mut i, &mut w, "*SRE?").as_deref(), Some("32"));
    }

    #[test]
    fn el_byte_de_estado_avisa_de_que_hay_errores() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        hablar(&mut i, &mut w, "*CLS");
        hablar(&mut i, &mut w, "NO:EXISTE");

        let s: u8 = hablar(&mut i, &mut w, "*STB?").unwrap().parse().unwrap();
        assert_ne!(s & (1 << stb::ERROR_QUEUE), 0);
    }

    #[test]
    fn opc_y_tst_contestan_lo_que_exige_la_norma() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        assert_eq!(hablar(&mut i, &mut w, "*OPC?").as_deref(), Some("1"));
        assert_eq!(hablar(&mut i, &mut w, "*TST?").as_deref(), Some("0"));
    }

    #[test]
    fn la_version_de_scpi_es_la_del_estandar() {
        let (mut i, mut w) = (Maniqui::new(), World::new());
        assert_eq!(
            hablar(&mut i, &mut w, "SYST:VERS?").as_deref(),
            Some("1999.0")
        );
    }
}
