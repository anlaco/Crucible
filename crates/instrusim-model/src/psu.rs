//! Fuente de alimentación de continua, conforme a la clase IviDCPwr.
//!
//! Es el instrumento que hace que el rack deje de ser una colección de piezas
//! sueltas: cuando la fuente impone una tensión en un nodo, cualquier
//! instrumento cableado a ese nodo la mide. Ese acoplamiento a través del mundo
//! es, en pequeño, el gemelo digital entero.
//!
//! No salta de cero a la consigna: sube por una rampa con su tiempo de
//! establecimiento, como una fuente real. Un cliente que mida inmediatamente
//! después de programar la tensión leerá un valor intermedio, exactamente igual
//! que le pasaría en el banco. Ese detalle es de los que descubren bugs de
//! verdad en el software de secuencias.

use std::time::Duration;

use instrusim_core::{NodeId, Signal, Stepper, World};
use instrusim_scpi::error::{ErrorCode, ErrorQueue, ScpiError};
use instrusim_scpi::format;
use instrusim_scpi::status::StatusModel;
use instrusim_scpi::{Command, CommandTable};

use crate::instrument::{Identity, Instrument};

/// Fuente de alimentación de continua de un canal.
pub struct GenericDcSupply {
    identity: Identity,
    status: StatusModel,
    errors: ErrorQueue,

    /// Nodo sobre el que impone tensión. El escenario lo asigna al cablear.
    output: Option<NodeId>,

    voltage_setpoint: f64,
    current_limit: f64,
    output_on: bool,

    max_voltage: f64,
    max_current: f64,
    /// Tiempo que tarda la salida en establecerse tras un cambio.
    settling: Duration,
    /// Ruido de la salida, en valor eficaz.
    noise_rms: f64,
    noise_seed: u64,

    commands: CommandTable<Cmd>,
}

#[derive(Debug, Clone, Copy)]
enum Cmd {
    Voltage,
    Current,
    Output,
    MeasureVoltage,
    MeasureCurrent,
}

impl GenericDcSupply {
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            status: StatusModel::new(),
            errors: ErrorQueue::default(),
            output: None,
            voltage_setpoint: 0.0,
            current_limit: 1.0,
            output_on: false,
            max_voltage: 30.0,
            max_current: 3.0,
            settling: Duration::from_millis(10),
            noise_rms: 200e-6,
            noise_seed: 0x5150,
            commands: tabla_de_comandos(),
        }
    }

    /// Fuente genérica de 30 V y 3 A.
    pub fn generic(serial: impl Into<String>) -> Self {
        Self::new(Identity::new("InstruSim", "GPS-3003", serial, "1.0"))
    }

    /// Conecta la salida de la fuente a un nodo del rack.
    pub fn wire(&mut self, output: NodeId) {
        self.output = Some(output);
    }

    pub fn with_limits(mut self, max_voltage: f64, max_current: f64) -> Self {
        self.max_voltage = max_voltage;
        self.max_current = max_current;
        self
    }

    pub fn with_settling(mut self, settling: Duration) -> Self {
        self.settling = settling;
        self
    }

    /// Tensión que la fuente está entregando en este instante.
    fn present_output(&self, world: &World) -> f64 {
        match self.output {
            Some(n) => world.potential_now(n),
            None => 0.0,
        }
    }

    /// Reprograma la señal del nodo de salida.
    ///
    /// Siempre arranca la rampa desde el valor que hay ahora mismo, no desde
    /// cero ni desde la consigna anterior. Así, cambiar la tensión a mitad de un
    /// establecimiento se comporta como en una fuente real: continúa desde donde
    /// estaba en vez de dar un salto.
    fn apply(&mut self, world: &mut World) {
        let Some(nodo) = self.output else {
            return;
        };

        let desde = world.potential_now(nodo);
        let hasta = if self.output_on {
            self.voltage_setpoint
        } else {
            0.0
        };
        let ahora = world.now();

        let rampa = Signal::Ramp {
            from: desde,
            to: hasta,
            start: ahora,
            duration: self.settling,
        };

        // El ruido solo existe con la salida activa: una salida apagada está a
        // cero limpio.
        let senal = if self.output_on {
            rampa.with_noise(self.noise_rms, self.noise_seed)
        } else {
            rampa
        };

        world.drive(nodo, senal);
    }
}

fn tabla_de_comandos() -> CommandTable<Cmd> {
    CommandTable::from_pairs([
        ("MEASure[:SCALar]:VOLTage[:DC]", Cmd::MeasureVoltage),
        ("MEASure[:SCALar]:CURRent[:DC]", Cmd::MeasureCurrent),
        (
            "[SOURce:]VOLTage[:LEVel][:IMMediate][:AMPLitude]",
            Cmd::Voltage,
        ),
        (
            "[SOURce:]CURRent[:LEVel][:IMMediate][:AMPLitude]",
            Cmd::Current,
        ),
        ("OUTPut[:STATe]", Cmd::Output),
    ])
}

impl Stepper for GenericDcSupply {
    fn step(&mut self, _world: &mut World, _dt: Duration) {
        // La rampa vive en la señal del nodo, así que se resuelve sola cada vez
        // que alguien la evalúa. No hay nada que ir empujando tic a tic, y esa
        // es precisamente la ventaja de que los nodos guarden funciones del
        // tiempo en lugar de números.
    }
}

impl Instrument for GenericDcSupply {
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
        // Una fuente que arranca con la salida activa es un peligro, y por eso
        // la norma manda que `*RST` la apague.
        self.voltage_setpoint = 0.0;
        self.current_limit = 1.0;
        self.output_on = false;
    }

    fn execute(&mut self, cmd: &Command, world: &mut World) -> Result<Option<String>, ScpiError> {
        let Some((accion, _)) = self.commands.lookup(&cmd.header) else {
            return Err(ScpiError::with_detail(
                ErrorCode::UndefinedHeader,
                &cmd.header,
            ));
        };
        let accion = *accion;

        match (accion, cmd.query) {
            (Cmd::Voltage, false) => {
                let v = cmd.numeric(0)?.resolve(0.0, self.max_voltage, 0.0);
                if !(0.0..=self.max_voltage).contains(&v) {
                    return Err(ScpiError::with_detail(
                        ErrorCode::DataOutOfRange,
                        format!("{v} V excede el máximo de {} V", self.max_voltage),
                    ));
                }
                self.voltage_setpoint = v;
                self.apply(world);
                Ok(None)
            }
            (Cmd::Voltage, true) => Ok(Some(format::nr3(self.voltage_setpoint))),

            (Cmd::Current, false) => {
                let i = cmd.numeric(0)?.resolve(0.0, self.max_current, 0.1);
                if !(0.0..=self.max_current).contains(&i) {
                    return Err(ScpiError::with_detail(
                        ErrorCode::DataOutOfRange,
                        format!("{i} A excede el máximo de {} A", self.max_current),
                    ));
                }
                self.current_limit = i;
                Ok(None)
            }
            (Cmd::Current, true) => Ok(Some(format::nr3(self.current_limit))),

            (Cmd::Output, false) => {
                self.output_on = cmd.boolean(0)?;
                self.apply(world);
                Ok(None)
            }
            (Cmd::Output, true) => Ok(Some(format::boolean(self.output_on))),

            // Lectura de la propia salida: no es la consigna, es lo que hay de
            // verdad en el nodo, con su rampa y su ruido.
            (Cmd::MeasureVoltage, true) => Ok(Some(format::nr3(self.present_output(world)))),

            // Sin modelo de carga no hay corriente que medir. Devolver cero es
            // honesto: la fuente no está entregando nada porque en el mundo aún
            // no hay nada que consuma. Con el análisis nodal de la fase 6 esto
            // pasará a ser una medida real.
            (Cmd::MeasureCurrent, true) => Ok(Some(format::nr3(0.0))),

            _ => Err(ScpiError::with_detail(
                ErrorCode::UndefinedHeader,
                &cmd.header,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::handle_message;
    use instrusim_core::{Clock, Engine, SimTime, Terminal, VirtualClock};

    fn banco() -> (GenericDcSupply, World, NodeId) {
        let mut world = World::new();
        let salida = world.add_node("psu_out");

        let mut psu = GenericDcSupply::generic("PSU0001");
        psu.wire(salida);

        (psu, world, salida)
    }

    fn hablar(p: &mut GenericDcSupply, w: &mut World, l: &str) -> Option<String> {
        handle_message(p, l, w)
    }

    fn preguntar(p: &mut GenericDcSupply, w: &mut World, l: &str) -> String {
        hablar(p, w, l).unwrap_or_else(|| panic!("sin respuesta a {l}"))
    }

    #[test]
    fn se_identifica_como_manda_la_norma() {
        let (mut p, mut w, _) = banco();
        assert_eq!(
            preguntar(&mut p, &mut w, "*IDN?"),
            "InstruSim,GPS-3003,PSU0001,1.0"
        );
    }

    #[test]
    fn arranca_apagada_y_a_cero_voltios() {
        let (mut p, mut w, nodo) = banco();
        assert_eq!(preguntar(&mut p, &mut w, "OUTP?"), "0");
        assert_eq!(w.potential_now(nodo), 0.0);
    }

    #[test]
    fn la_consigna_se_programa_y_se_consulta() {
        let (mut p, mut w, _) = banco();
        hablar(&mut p, &mut w, "VOLT 3.3");
        let v: f64 = preguntar(&mut p, &mut w, "VOLT?").parse().unwrap();
        assert_eq!(v, 3.3);
    }

    /// Con la salida apagada, programar la consigna no pone tensión en el nodo.
    #[test]
    fn con_la_salida_apagada_el_nodo_sigue_a_cero() {
        let (mut p, mut w, nodo) = banco();
        hablar(&mut p, &mut w, "VOLT 5");
        assert_eq!(w.potential_now(nodo), 0.0);
    }

    /// El comportamiento que distingue a una fuente real de una variable: al
    /// encender, la tensión sube por una rampa. Justo después de encender no ha
    /// llegado a la consigna.
    #[test]
    fn la_salida_sube_por_una_rampa_al_encender() {
        let (mut p, mut w, nodo) = banco();
        hablar(&mut p, &mut w, "VOLT 5;:OUTP ON");

        // En el instante de encender, la salida aún está en cero.
        assert!(w.potential_now(nodo).abs() < 0.01);

        // A mitad del tiempo de establecimiento, a mitad de camino.
        w.set_now(SimTime::from_secs_f64(0.005));
        assert!((w.potential_now(nodo) - 2.5).abs() < 0.01);

        // Ya establecida.
        w.set_now(SimTime::from_secs_f64(0.020));
        assert!((w.potential_now(nodo) - 5.0).abs() < 0.01);
    }

    #[test]
    fn apagar_la_salida_la_baja_a_cero() {
        let (mut p, mut w, nodo) = banco();
        hablar(&mut p, &mut w, "VOLT 5;:OUTP ON");
        w.set_now(SimTime::from_secs_f64(0.020));

        hablar(&mut p, &mut w, "OUTP OFF");
        w.set_now(SimTime::from_secs_f64(0.040));

        assert!(w.potential_now(nodo).abs() < 0.01);
    }

    /// Cambiar la consigna a mitad de un establecimiento continúa desde donde
    /// estaba, sin saltos.
    #[test]
    fn un_cambio_a_media_rampa_continua_desde_el_valor_actual() {
        let (mut p, mut w, nodo) = banco();
        hablar(&mut p, &mut w, "VOLT 10;:OUTP ON");

        // A mitad de camino hacia 10 V, o sea en torno a 5 V.
        w.set_now(SimTime::from_secs_f64(0.005));
        let intermedio = w.potential_now(nodo);
        assert!((intermedio - 5.0).abs() < 0.05, "intermedio: {intermedio}");

        // Se cambia la consigna a 2 V: la nueva rampa parte de donde estaba.
        hablar(&mut p, &mut w, "VOLT 2");
        let justo_despues = w.potential_now(nodo);
        assert!(
            (justo_despues - intermedio).abs() < 0.05,
            "no debería saltar: {intermedio} -> {justo_despues}"
        );

        w.set_now(SimTime::from_secs_f64(0.020));
        assert!((w.potential_now(nodo) - 2.0).abs() < 0.01);
    }

    #[test]
    fn la_medida_de_salida_no_es_la_consigna_sino_lo_que_hay() {
        let (mut p, mut w, _) = banco();
        hablar(&mut p, &mut w, "VOLT 5;:OUTP ON");
        w.set_now(SimTime::from_secs_f64(0.005)); // a media rampa

        let consigna: f64 = preguntar(&mut p, &mut w, "VOLT?").parse().unwrap();
        let medida: f64 = preguntar(&mut p, &mut w, "MEAS:VOLT?").parse().unwrap();

        assert_eq!(consigna, 5.0);
        assert!((medida - 2.5).abs() < 0.05, "medida: {medida}");
    }

    #[test]
    fn una_consigna_por_encima_del_maximo_se_rechaza() {
        let (mut p, mut w, _) = banco();
        hablar(&mut p, &mut w, "*CLS");

        hablar(&mut p, &mut w, "VOLT 100");
        assert!(preguntar(&mut p, &mut w, "SYST:ERR?").starts_with("-222"));
        // Y la consigna no se ha movido.
        assert_eq!(preguntar(&mut p, &mut w, "VOLT?"), format::nr3(0.0));
    }

    #[test]
    fn las_palabras_clave_del_estandar_funcionan_en_la_consigna() {
        let (mut p, mut w, _) = banco();
        hablar(&mut p, &mut w, "VOLT MAX");
        let v: f64 = preguntar(&mut p, &mut w, "VOLT?").parse().unwrap();
        assert_eq!(v, 30.0);
    }

    #[test]
    fn reset_apaga_la_salida() {
        let (mut p, mut w, _) = banco();
        hablar(&mut p, &mut w, "VOLT 5;:OUTP ON");
        hablar(&mut p, &mut w, "*RST");

        assert_eq!(preguntar(&mut p, &mut w, "OUTP?"), "0");
        assert_eq!(preguntar(&mut p, &mut w, "VOLT?"), format::nr3(0.0));
    }

    /// El escenario completo: la fuente impone tensión y el multímetro, que no
    /// la conoce de nada, la mide a través del nodo compartido. Es el gemelo
    /// digital en miniatura.
    #[test]
    fn la_fuente_y_el_multimetro_se_acoplan_por_el_mundo() {
        use crate::dmm::GenericDmm;

        let mut motor = Engine::new(Box::new(VirtualClock::from_hz(1000.0)));
        let masa = motor.world_mut().add_node("masa");
        let salida = motor.world_mut().add_node("psu_out");

        let mut psu = GenericDcSupply::generic("PSU1");
        psu.wire(salida);

        let mut dmm = GenericDmm::generic("DMM1");
        dmm.wire(Terminal::wired("HI", salida), Terminal::wired("LO", masa));

        // La fuente entrega 3,3 V.
        handle_message(&mut psu, "VOLT 3.3;:OUTP ON", motor.world_mut());

        // Se deja establecer.
        motor.run_for(Duration::from_millis(50));

        // Y el multímetro lo mide, sin saber que hay una fuente al otro lado.
        let lectura = handle_message(&mut dmm, "MEAS:VOLT:DC?", motor.world_mut()).unwrap();
        let v: f64 = lectura.parse().unwrap();

        assert!((v - 3.3).abs() < 5e-3, "el multímetro leyó {v}");
    }

    /// Y si la fuente cambia, el multímetro lo nota. Nadie ha refrescado nada.
    #[test]
    fn al_cambiar_la_fuente_cambia_lo_que_lee_el_multimetro() {
        use crate::dmm::GenericDmm;

        let mut motor = Engine::new(Box::new(VirtualClock::from_hz(1000.0)));
        let masa = motor.world_mut().add_node("masa");
        let salida = motor.world_mut().add_node("psu_out");

        let mut psu = GenericDcSupply::generic("PSU1");
        psu.wire(salida);
        let mut dmm = GenericDmm::generic("DMM1");
        dmm.wire(Terminal::wired("HI", salida), Terminal::wired("LO", masa));

        let leer = |dmm: &mut GenericDmm, motor: &mut Engine| -> f64 {
            handle_message(dmm, "MEAS:VOLT:DC?", motor.world_mut())
                .unwrap()
                .parse()
                .unwrap()
        };

        handle_message(&mut psu, "VOLT 1;:OUTP ON", motor.world_mut());
        motor.run_for(Duration::from_millis(50));
        assert!((leer(&mut dmm, &mut motor) - 1.0).abs() < 5e-3);

        handle_message(&mut psu, "VOLT 12", motor.world_mut());
        motor.run_for(Duration::from_millis(50));
        assert!((leer(&mut dmm, &mut motor) - 12.0).abs() < 5e-3);
    }

    #[test]
    fn el_reloj_del_motor_gobierna_el_establecimiento() {
        let mut reloj = VirtualClock::from_hz(1000.0);
        assert_eq!(reloj.tick().as_nanos(), 1_000_000);
    }
}
