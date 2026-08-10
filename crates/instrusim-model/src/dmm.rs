//! Multímetro genérico, conforme a la clase IviDmm.
//!
//! No imita a ningún modelo comercial: implementa el árbol SCPI que la clase
//! define, que es el mismo en un Keysight y en un Rohde & Schwarz salvo por las
//! extensiones propias de cada uno.
//!
//! Lo que lo separa de un simulador de juguete es que **no inventa el valor que
//! devuelve**. Lee la diferencia de potencial entre sus bornes en el mundo, y
//! después le aplica lo que un multímetro real le hace a esa magnitud: error de
//! ganancia y de cero de su calibración, cuantización a la resolución del rango
//! elegido, y desbordamiento si no cabe. Cuando en la fase 6 la tensión del nodo
//! la calcule el análisis nodal en vez del escenario, este código no cambia.

use std::time::Duration;

use instrusim_core::{SimTime, Stepper, Terminal, World};
use instrusim_scpi::error::{ErrorCode, ErrorQueue, ScpiError};
use instrusim_scpi::format::{self, OVERFLOW};
use instrusim_scpi::status::StatusModel;
use instrusim_scpi::{Command, CommandTable};

use crate::instrument::{Identity, Instrument};

/// Función de medida seleccionada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    VoltageDc,
    VoltageAc,
}

impl Function {
    /// El nombre con que `FUNCtion?` la devuelve, entre comillas como exige
    /// SCPI para los parámetros de tipo cadena.
    fn name(self) -> &'static str {
        match self {
            Function::VoltageDc => "VOLT:DC",
            Function::VoltageAc => "VOLT:AC",
        }
    }

    /// Rangos disponibles, de menor a mayor.
    fn ranges(self) -> &'static [f64] {
        match self {
            Function::VoltageDc => &[0.1, 1.0, 10.0, 100.0, 1000.0],
            Function::VoltageAc => &[0.1, 1.0, 10.0, 100.0, 750.0],
        }
    }
}

/// Errores de calibración del ejemplar concreto.
///
/// Son fijos, no aleatorios: un multímetro tiene *su* desviación, la misma en
/// cada medida, y por eso se calibra. El ruido, que sí varía, ya viene en la
/// señal del nodo.
#[derive(Debug, Clone, Copy)]
pub struct Accuracy {
    /// Error proporcional a la lectura, en partes por millón.
    pub gain_ppm: f64,
    /// Error de cero, en partes por millón del rango.
    pub offset_ppm_range: f64,
}

impl Default for Accuracy {
    /// Cifras de un multímetro de sobremesa de seis dígitos y medio.
    fn default() -> Self {
        Self {
            gain_ppm: 30.0,
            offset_ppm_range: 7.0,
        }
    }
}

/// Multímetro digital genérico.
pub struct GenericDmm {
    identity: Identity,
    status: StatusModel,
    errors: ErrorQueue,

    /// Bornes de entrada. El escenario los cablea a los nodos del rack.
    pub hi: Terminal,
    pub lo: Terminal,

    function: Function,
    /// Rango fijo, o `None` si está en automático.
    range: Option<f64>,
    /// Tiempo de integración en ciclos de red.
    nplc: f64,
    /// Frecuencia de red, que fija cuánto dura un NPLC.
    line_frequency: f64,
    accuracy: Accuracy,

    /// Última lectura, la que devuelve `FETCh?`.
    last_reading: Option<f64>,

    commands: CommandTable<Cmd>,
}

/// Las acciones que entiende este instrumento.
#[derive(Debug, Clone, Copy)]
enum Cmd {
    MeasureDc,
    MeasureAc,
    Configure,
    ConfigureQuery,
    Read,
    Initiate,
    Fetch,
    FunctionSet,
    Range,
    RangeAuto,
    Nplc,
}

impl GenericDmm {
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            status: StatusModel::new(),
            errors: ErrorQueue::default(),
            hi: Terminal::floating("HI"),
            lo: Terminal::floating("LO"),
            function: Function::VoltageDc,
            range: None,
            nplc: 1.0,
            line_frequency: 50.0,
            accuracy: Accuracy::default(),
            last_reading: None,
            commands: tabla_de_comandos(),
        }
    }

    /// Multímetro genérico con la identidad por defecto.
    pub fn generic(serial: impl Into<String>) -> Self {
        Self::new(Identity::new("InstruSim", "GDM-1000", serial, "1.0"))
    }

    pub fn with_accuracy(mut self, accuracy: Accuracy) -> Self {
        self.accuracy = accuracy;
        self
    }

    /// Cablea los bornes del multímetro a dos nodos del rack.
    pub fn wire(&mut self, hi: Terminal, lo: Terminal) {
        self.hi = hi;
        self.lo = lo;
    }

    /// Cuánto dura una medida con la integración configurada.
    fn aperture(&self) -> Duration {
        Duration::from_secs_f64(self.nplc / self.line_frequency)
    }

    /// El rango en uso: el fijado, o el que elegiría el automático.
    fn active_range(&self, valor: f64) -> f64 {
        match self.range {
            Some(r) => r,
            None => {
                let rangos = self.function.ranges();
                *rangos
                    .iter()
                    .find(|r| valor.abs() <= **r)
                    .unwrap_or_else(|| rangos.last().expect("siempre hay rangos"))
            }
        }
    }

    /// Toma una medida de verdad, con todo lo que un multímetro le hace a la
    /// señal que tiene delante.
    ///
    /// La integración va **hacia delante** desde el instante actual: el valor
    /// devuelto es el que daría un instrumento real que empezase a integrar
    /// ahora mismo y tardase su tiempo de apertura en contestar. Tiene una
    /// consecuencia que sorprende la primera vez y que es fiel al mundo real:
    /// con la integración por defecto de 1 NPLC —20 ms a 50 Hz— la ventana de
    /// medida se traga los transitorios cortos y los promedia. Para ver un
    /// establecimiento de 10 ms hay que bajar el NPLC, exactamente igual que en
    /// el banco.
    fn measure(&mut self, world: &World) -> f64 {
        let bruto = match self.function {
            Function::VoltageDc => self.integrate_dc(world),
            Function::VoltageAc => self.true_rms(world),
        };

        let rango = self.active_range(bruto);

        // Desbordamiento: los multímetros admiten algo por encima del rango
        // nominal antes de rendirse, típicamente un 20%.
        if bruto.abs() > rango * 1.2 {
            self.last_reading = Some(OVERFLOW);
            return OVERFLOW;
        }

        // Error de calibración del ejemplar: ganancia y cero.
        let con_error = bruto * (1.0 + self.accuracy.gain_ppm * 1e-6)
            + rango * self.accuracy.offset_ppm_range * 1e-6;

        // Cuantización a la resolución del convertidor: seis dígitos y medio
        // sobre el rango en uso.
        let cuanto = rango / 2_000_000.0;
        let cuantizado = (con_error / cuanto).round() * cuanto;

        self.last_reading = Some(cuantizado);
        cuantizado
    }

    /// Medida en continua: la media de la señal durante el tiempo de
    /// integración.
    ///
    /// Integrar en lugar de tomar un único punto no es un adorno. Es lo que
    /// hace un multímetro real y es la razón de que el ruido no le afecte tanto
    /// como cabría esperar: promediar N muestras lo reduce en raíz de N. Un
    /// simulador que devolviese el valor instantáneo daría lecturas mucho más
    /// ruidosas que el instrumento al que imita.
    fn integrate_dc(&self, world: &World) -> f64 {
        let muestras = 256;
        let apertura = self.aperture();
        let paso = Duration::from_nanos((apertura.as_nanos() as u64 / muestras as u64).max(1));

        let inicio = world.now();
        let mut suma = 0.0;
        for i in 0..muestras {
            let t = inicio + paso * i as u32;
            suma += world.differential_at(&self.hi, &self.lo, t);
        }
        suma / muestras as f64
    }

    /// Medida en alterna: valor eficaz verdadero, sin la componente continua.
    ///
    /// Aquí es donde se ve para qué servía que los nodos guarden señales y no
    /// números: el instrumento muestrea la ventana de integración a la
    /// resolución que le conviene, muy por encima del ritmo del motor, y el
    /// resultado es el valor eficaz correcto de una senoide de la frecuencia
    /// que sea.
    fn true_rms(&self, world: &World) -> f64 {
        let muestras = 4096;
        let apertura = self.aperture();
        let paso = Duration::from_nanos((apertura.as_nanos() as u64 / muestras as u64).max(1));

        let inicio = world.now();
        let mut valores = Vec::with_capacity(muestras);
        for i in 0..muestras {
            let t = inicio + paso * i as u32;
            valores.push(world.differential_at(&self.hi, &self.lo, t));
        }

        let media = valores.iter().sum::<f64>() / muestras as f64;
        let varianza = valores
            .iter()
            .map(|v| (v - media) * (v - media))
            .sum::<f64>()
            / muestras as f64;

        varianza.sqrt()
    }
}

/// El árbol SCPI de la clase IviDmm.
fn tabla_de_comandos() -> CommandTable<Cmd> {
    CommandTable::from_pairs([
        // El orden importa: lo específico antes que lo general.
        ("MEASure[:SCALar]:VOLTage:AC", Cmd::MeasureAc),
        ("MEASure[:SCALar][:VOLTage][:DC]", Cmd::MeasureDc),
        ("CONFigure[:SCALar][:VOLTage][:DC]", Cmd::Configure),
        ("CONFigure", Cmd::ConfigureQuery),
        ("READ", Cmd::Read),
        ("INITiate[:IMMediate]", Cmd::Initiate),
        ("FETCh[:SCALar]", Cmd::Fetch),
        ("[SENSe:]FUNCtion[:ON]", Cmd::FunctionSet),
        ("[SENSe:]VOLTage[:DC]:RANGe:AUTO", Cmd::RangeAuto),
        ("[SENSe:]VOLTage[:DC]:RANGe[:UPPer]", Cmd::Range),
        ("[SENSe:]VOLTage[:DC]:NPLCycles", Cmd::Nplc),
    ])
}

impl Stepper for GenericDmm {
    fn step(&mut self, _world: &mut World, _dt: Duration) {
        // Sin comandos solapados no hay nada que avanzar entre tics: la medida
        // se resuelve entera dentro del comando que la pide. El gancho queda
        // puesto para cuando haya medidas disparadas y en segundo plano.
    }
}

impl Instrument for GenericDmm {
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
        // `*RST` devuelve la configuración al encendido, pero no toca el
        // cableado: los cables no se desconectan solos.
        self.function = Function::VoltageDc;
        self.range = None;
        self.nplc = 1.0;
        self.last_reading = None;
    }

    fn execute(&mut self, cmd: &Command, world: &mut World) -> Result<Option<String>, ScpiError> {
        let Some((accion, _sufijos)) = self.commands.lookup(&cmd.header) else {
            return Err(ScpiError::with_detail(
                ErrorCode::UndefinedHeader,
                &cmd.header,
            ));
        };
        let accion = *accion;

        match (accion, cmd.query) {
            // MEASure? equivale a configurar y leer de una vez.
            (Cmd::MeasureDc, true) => {
                self.function = Function::VoltageDc;
                aplicar_rango(self, cmd)?;
                Ok(Some(format::nr3(self.measure(world))))
            }
            (Cmd::MeasureAc, true) => {
                self.function = Function::VoltageAc;
                aplicar_rango(self, cmd)?;
                Ok(Some(format::nr3(self.measure(world))))
            }

            (Cmd::Configure, false) => {
                self.function = Function::VoltageDc;
                aplicar_rango(self, cmd)?;
                self.last_reading = None;
                Ok(None)
            }

            (Cmd::ConfigureQuery, true) => {
                let rango = self
                    .range
                    .unwrap_or_else(|| *self.function.ranges().last().expect("siempre hay rangos"));
                Ok(Some(format!(
                    "\"{} {},{}\"",
                    self.function.name(),
                    format::nr3(rango),
                    format::nr3(rango / 2_000_000.0)
                )))
            }

            // READ? es INITiate seguido de FETCh?.
            (Cmd::Read, true) => Ok(Some(format::nr3(self.measure(world)))),

            (Cmd::Initiate, false) => {
                self.measure(world);
                Ok(None)
            }

            (Cmd::Fetch, true) => match self.last_reading {
                Some(v) => Ok(Some(format::nr3(v))),
                // Pedir un resultado que no se ha disparado es un error de
                // consulta con nombre propio en el estándar.
                None => Err(ScpiError::with_detail(
                    ErrorCode::SettingsConflict,
                    "no hay medida pendiente; use INIT o READ?",
                )),
            },

            (Cmd::FunctionSet, false) => {
                let arg = cmd.arg(0)?.trim_matches('"').to_ascii_uppercase();
                self.function = match arg.as_str() {
                    "VOLT" | "VOLT:DC" | "VOLTAGE:DC" | "VOLTAGE" => Function::VoltageDc,
                    "VOLT:AC" | "VOLTAGE:AC" => Function::VoltageAc,
                    otro => {
                        return Err(ScpiError::with_detail(
                            ErrorCode::IllegalParameterValue,
                            otro,
                        ));
                    }
                };
                Ok(None)
            }
            (Cmd::FunctionSet, true) => Ok(Some(format!("\"{}\"", self.function.name()))),

            (Cmd::Range, false) => {
                let rangos = self.function.ranges();
                let minimo = rangos[0];
                let maximo = *rangos.last().expect("siempre hay rangos");
                let pedido = cmd.numeric(0)?.resolve(minimo, maximo, maximo);

                if pedido < 0.0 || pedido > maximo {
                    return Err(ScpiError::new(ErrorCode::DataOutOfRange));
                }
                // Se sube al primer rango que dé cabida al valor pedido, que es
                // lo que hacen los instrumentos reales.
                self.range = Some(*rangos.iter().find(|r| pedido <= **r).unwrap_or(&maximo));
                Ok(None)
            }
            (Cmd::Range, true) => {
                let r = self
                    .range
                    .unwrap_or_else(|| *self.function.ranges().last().expect("hay rangos"));
                Ok(Some(format::nr3(r)))
            }

            (Cmd::RangeAuto, false) => {
                self.range = if cmd.boolean(0)? {
                    None
                } else {
                    Some(self.active_range(0.0))
                };
                Ok(None)
            }
            (Cmd::RangeAuto, true) => Ok(Some(format::boolean(self.range.is_none()))),

            (Cmd::Nplc, false) => {
                let v = cmd.numeric(0)?.resolve(0.02, 100.0, 1.0);
                if !(0.02..=100.0).contains(&v) {
                    return Err(ScpiError::new(ErrorCode::DataOutOfRange));
                }
                self.nplc = v;
                Ok(None)
            }
            (Cmd::Nplc, true) => Ok(Some(format::nr3(self.nplc))),

            // La cabecera existe pero no admite esa forma: consultar algo que
            // solo es orden, o dar una orden a algo que solo es consulta.
            _ => Err(ScpiError::with_detail(
                ErrorCode::UndefinedHeader,
                &cmd.header,
            )),
        }
    }
}

/// `MEASure?` y `CONFigure` aceptan rango y resolución como argumentos
/// opcionales. Si no vienen, se queda como estaba.
fn aplicar_rango(dmm: &mut GenericDmm, cmd: &Command) -> Result<(), ScpiError> {
    if cmd.args.is_empty() {
        return Ok(());
    }

    let rangos = dmm.function.ranges();
    let maximo = *rangos.last().expect("siempre hay rangos");

    match cmd.numeric(0)? {
        instrusim_scpi::Numeric::Value(v) if v >= 0.0 => {
            dmm.range = Some(*rangos.iter().find(|r| v <= **r).unwrap_or(&maximo));
        }
        instrusim_scpi::Numeric::Value(_) => return Err(ScpiError::new(ErrorCode::DataOutOfRange)),
        // AUTO viene expresado como MINimum/MAXimum/DEFault según el fabricante;
        // aquí cualquiera de las tres deja el rango en automático.
        _ => dmm.range = None,
    }

    Ok(())
}

/// Utilidad para el escenario: el instante en que una medida terminaría.
pub fn measurement_end(start: SimTime, nplc: f64, line_frequency: f64) -> SimTime {
    start + Duration::from_secs_f64(nplc / line_frequency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::handle_message;
    use instrusim_core::Signal;

    /// Monta un mundo con una señal en el nodo y un multímetro midiéndolo.
    fn banco(senal: Signal) -> (GenericDmm, World) {
        let mut world = World::new();
        let masa = world.add_node("masa");
        let punto = world.add_node_with("punto", senal);

        let mut dmm = GenericDmm::generic("SIM0001");
        dmm.wire(Terminal::wired("HI", punto), Terminal::wired("LO", masa));

        (dmm, world)
    }

    fn preguntar(dmm: &mut GenericDmm, world: &mut World, linea: &str) -> String {
        handle_message(dmm, linea, world).unwrap_or_else(|| panic!("sin respuesta a {linea}"))
    }

    fn valor(dmm: &mut GenericDmm, world: &mut World, linea: &str) -> f64 {
        preguntar(dmm, world, linea).parse().expect("número NR3")
    }

    #[test]
    fn se_identifica_como_manda_la_norma() {
        let (mut d, mut w) = banco(Signal::ZERO);
        assert_eq!(
            preguntar(&mut d, &mut w, "*IDN?"),
            "InstruSim,GDM-1000,SIM0001,1.0"
        );
    }

    #[test]
    fn mide_la_tension_que_hay_en_sus_bornes() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        let v = valor(&mut d, &mut w, "MEAS:VOLT:DC?");
        assert!((v - 5.0).abs() < 1e-3, "medida: {v}");
    }

    /// La propiedad central: el multímetro no inventa nada. Si cambia lo que
    /// hay en el nodo, cambia la lectura.
    #[test]
    fn la_lectura_sigue_a_lo_que_pasa_en_el_mundo() {
        let (mut d, mut w) = banco(Signal::Constant(1.0));
        assert!((valor(&mut d, &mut w, "MEAS:VOLT:DC?") - 1.0).abs() < 1e-3);

        let punto = w.node_by_name("punto").unwrap();
        w.drive(punto, Signal::Constant(-2.5));

        assert!((valor(&mut d, &mut w, "MEAS:VOLT:DC?") + 2.5).abs() < 1e-3);
    }

    #[test]
    fn la_forma_corta_y_la_larga_son_equivalentes() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        let a = preguntar(&mut d, &mut w, "MEAS:VOLT:DC?");
        let b = preguntar(&mut d, &mut w, "measure:voltage:dc?");
        let c = preguntar(&mut d, &mut w, "MEAS?"); // tramos opcionales omitidos
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn la_respuesta_va_en_el_formato_nr3_del_estandar() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        let r = preguntar(&mut d, &mut w, "MEAS:VOLT:DC?");
        assert!(r.starts_with('+'), "falta el signo: {r}");
        assert!(r.contains("E+"), "falta el exponente: {r}");
    }

    #[test]
    fn una_tension_fuera_de_rango_devuelve_desbordamiento() {
        let (mut d, mut w) = banco(Signal::Constant(50.0));
        handle_message(&mut d, "*CLS", &mut w);
        handle_message(&mut d, "VOLT:DC:RANG 1", &mut w);

        let v = valor(&mut d, &mut w, "READ?");
        assert!(v > 1e37, "debería desbordar: {v}");
    }

    #[test]
    fn el_rango_automatico_elige_el_mas_pequeno_que_sirve() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        // 5 V no cabe en el rango de 1 V, así que toca el de 10 V.
        preguntar(&mut d, &mut w, "MEAS:VOLT:DC?");
        assert_eq!(valor(&mut d, &mut w, "VOLT:DC:RANG?"), 1000.0); // sin fijar, informa el máximo

        handle_message(&mut d, "VOLT:DC:RANG 5", &mut w);
        assert_eq!(valor(&mut d, &mut w, "VOLT:DC:RANG?"), 10.0);
    }

    #[test]
    fn el_rango_automatico_se_puede_encender_y_apagar() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        assert_eq!(preguntar(&mut d, &mut w, "VOLT:DC:RANG:AUTO?"), "1");

        handle_message(&mut d, "VOLT:DC:RANG 10", &mut w);
        assert_eq!(preguntar(&mut d, &mut w, "VOLT:DC:RANG:AUTO?"), "0");

        handle_message(&mut d, "VOLT:DC:RANG:AUTO ON", &mut w);
        assert_eq!(preguntar(&mut d, &mut w, "VOLT:DC:RANG:AUTO?"), "1");
    }

    #[test]
    fn init_y_fetch_hacen_lo_mismo_que_read_en_dos_pasos() {
        let (mut d, mut w) = banco(Signal::Constant(3.3));
        handle_message(&mut d, "INIT", &mut w);
        let v: f64 = preguntar(&mut d, &mut w, "FETC?").parse().unwrap();
        assert!((v - 3.3).abs() < 1e-3);
    }

    #[test]
    fn fetch_sin_medida_previa_es_un_error() {
        let (mut d, mut w) = banco(Signal::Constant(3.3));
        handle_message(&mut d, "*CLS", &mut w);

        assert_eq!(handle_message(&mut d, "FETC?", &mut w), None);
        assert!(preguntar(&mut d, &mut w, "SYST:ERR?").starts_with("-221"));
    }

    #[test]
    fn la_funcion_se_puede_consultar_y_cambiar() {
        let (mut d, mut w) = banco(Signal::ZERO);
        assert_eq!(preguntar(&mut d, &mut w, "FUNC?"), "\"VOLT:DC\"");

        handle_message(&mut d, "FUNC \"VOLT:AC\"", &mut w);
        assert_eq!(preguntar(&mut d, &mut w, "FUNC?"), "\"VOLT:AC\"");
    }

    /// El test que justifica el diseño de las señales. El motor no ha corrido
    /// ni un tic, pero el multímetro obtiene el valor eficaz correcto de una
    /// senoide de 1 kHz muestreando su ventana de integración.
    #[test]
    fn mide_el_valor_eficaz_verdadero_de_una_senoide() {
        // 10 V de pico a 1 kHz: el eficaz son 10/raíz(2) = 7,071 V.
        let (mut d, mut w) = banco(Signal::sine(10.0, 1000.0));

        handle_message(&mut d, "FUNC \"VOLT:AC\"", &mut w);
        let v = valor(&mut d, &mut w, "READ?");

        assert!((v - 7.0711).abs() < 0.01, "eficaz medido: {v}");
    }

    #[test]
    fn el_eficaz_no_cuenta_la_componente_continua() {
        // Senoide de 10 V de pico montada sobre 5 V de continua.
        let senal = Signal::Sine {
            amplitude: 10.0,
            frequency: 1000.0,
            phase: 0.0,
            offset: 5.0,
        };
        let (mut d, mut w) = banco(senal);

        handle_message(&mut d, "FUNC \"VOLT:AC\"", &mut w);
        let v = valor(&mut d, &mut w, "READ?");

        assert!((v - 7.0711).abs() < 0.01, "eficaz medido: {v}");
    }

    /// La integración es lo que hace que un multímetro real sea inmune al
    /// ruido: promediar la ventana lo reduce en raíz del número de muestras.
    #[test]
    fn la_integracion_reduce_el_ruido() {
        let ruidosa = Signal::Constant(1.0).with_noise(0.01, 7); // 10 mV de ruido
        let (mut d, mut w) = banco(ruidosa);

        let v = valor(&mut d, &mut w, "MEAS:VOLT:DC?");
        let error = (v - 1.0).abs();

        // Con 256 muestras, el error debe quedar muy por debajo del ruido
        // instantáneo de 10 mV.
        assert!(error < 2e-3, "el promediado no redujo el ruido: {error}");
        assert!(error > 0.0, "debería haber algo de error residual");
    }

    #[test]
    fn el_tiempo_de_integracion_se_configura_y_se_consulta() {
        let (mut d, mut w) = banco(Signal::ZERO);
        handle_message(&mut d, "VOLT:DC:NPLC 10", &mut w);
        assert_eq!(valor(&mut d, &mut w, "VOLT:DC:NPLC?"), 10.0);
    }

    #[test]
    fn un_tiempo_de_integracion_imposible_se_rechaza() {
        let (mut d, mut w) = banco(Signal::ZERO);
        handle_message(&mut d, "*CLS", &mut w);

        handle_message(&mut d, "VOLT:DC:NPLC 5000", &mut w);
        assert!(preguntar(&mut d, &mut w, "SYST:ERR?").starts_with("-222"));
    }

    #[test]
    fn reset_devuelve_la_configuracion_al_encendido_sin_descablear() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        handle_message(&mut d, "FUNC \"VOLT:AC\";:VOLT:DC:NPLC 10", &mut w);
        handle_message(&mut d, "*RST", &mut w);

        assert_eq!(preguntar(&mut d, &mut w, "FUNC?"), "\"VOLT:DC\"");
        assert_eq!(valor(&mut d, &mut w, "VOLT:DC:NPLC?"), 1.0);
        // Y sigue midiendo: el cableado no se ha tocado.
        assert!((valor(&mut d, &mut w, "READ?") - 5.0).abs() < 1e-3);
    }

    #[test]
    fn dos_lecturas_del_mismo_valor_estable_coinciden() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        let a = preguntar(&mut d, &mut w, "READ?");
        let b = preguntar(&mut d, &mut w, "READ?");
        assert_eq!(a, b, "sin ruido, la medida debe ser repetible");
    }

    /// El error de calibración es sistemático, no aleatorio: siempre el mismo,
    /// que es justo lo que permite corregirlo calibrando.
    #[test]
    fn el_error_de_calibracion_es_sistematico() {
        let (mut d, mut w) = banco(Signal::Constant(5.0));
        let v = valor(&mut d, &mut w, "MEAS:VOLT:DC?");

        assert_ne!(v, 5.0, "un multímetro real no da el valor exacto");
        assert!((v - 5.0).abs() < 1e-3, "pero se le parece mucho: {v}");
    }
}
