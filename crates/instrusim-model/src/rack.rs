//! El rack: el conjunto de instrumentos que comparten un mundo y un reloj.
//!
//! Es lo que en la fase 3 se cargará de un fichero de escenario. Por ahora se
//! monta a mano desde el código, pero la forma ya es la definitiva: instrumentos
//! con identificador, nodos cableados entre ellos y un único reloj para todos.
//!
//! Un detalle que parece menor y no lo es: **un solo hilo posee el rack entero**
//! y es el único que lo toca. Los servidores de red no comparten estado con él,
//! le mandan mensajes. Por eso no hay cerrojos en ninguna parte del proyecto, y
//! por eso dos instrumentos nunca pueden ver el mundo en instantes distintos.

use std::time::Duration;

use instrusim_core::{Clock, SimTime, World};

use crate::instrument::{Instrument, handle_message};

/// Identificador de un instrumento dentro del rack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstrumentId(pub usize);

/// Un conjunto de instrumentos simulados sobre un mundo común.
pub struct Rack {
    world: World,
    clock: Box<dyn Clock>,
    instruments: Vec<Box<dyn Instrument>>,
}

impl Rack {
    pub fn new(clock: Box<dyn Clock>) -> Self {
        let mut world = World::new();
        world.set_now(clock.now());
        Self {
            world,
            clock,
            instruments: Vec::new(),
        }
    }

    /// Incorpora un instrumento y devuelve su identificador.
    pub fn add(&mut self, instrument: Box<dyn Instrument>) -> InstrumentId {
        self.instruments.push(instrument);
        InstrumentId(self.instruments.len() - 1)
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn now(&self) -> SimTime {
        self.world.now()
    }

    pub fn len(&self) -> usize {
        self.instruments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instruments.is_empty()
    }

    /// La identidad de un instrumento, para los mensajes de arranque.
    pub fn idn(&self, id: InstrumentId) -> Option<String> {
        self.instruments.get(id.0).map(|i| i.identity().idn())
    }

    /// Avanza la simulación un tic.
    ///
    /// Con reloj de pared bloquea hasta que llega el momento; con reloj virtual
    /// vuelve al instante.
    pub fn tick(&mut self) -> SimTime {
        let ahora = self.clock.tick();
        let dt = self.clock.period();

        // El instante se fija antes de que nadie se ejecute, para que todos los
        // instrumentos vean el mismo "ahora" dentro del tic.
        self.world.set_now(ahora);

        for instrumento in self.instruments.iter_mut() {
            instrumento.step(&mut self.world, dt);
        }

        ahora
    }

    pub fn run_for(&mut self, duration: Duration) {
        let hasta = self.world.now() + duration;
        while self.world.now() < hasta {
            self.tick();
        }
    }

    /// Entrega una línea SCPI a un instrumento y devuelve lo que conteste.
    ///
    /// `None` significa que el mensaje no contenía ninguna consulta, así que no
    /// hay nada que enviar de vuelta. Es importante distinguirlo de una
    /// respuesta vacía: un cliente que espere respuesta a un comando que no la
    /// tiene se quedará colgado hasta agotar su tiempo de espera, exactamente
    /// igual que le pasaría con el instrumento real.
    pub fn dispatch(&mut self, id: InstrumentId, line: &str) -> Option<String> {
        let instrumento = self.instruments.get_mut(id.0)?;
        handle_message(instrumento.as_mut(), line, &mut self.world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GenericDcSupply, GenericDmm};
    use instrusim_core::{Terminal, VirtualClock};

    /// Monta el rack de demostración: una fuente alimentando un nodo y un
    /// multímetro midiéndolo.
    fn rack_de_demostracion() -> (Rack, InstrumentId, InstrumentId) {
        let mut rack = Rack::new(Box::new(VirtualClock::from_hz(1000.0)));

        let masa = rack.world_mut().add_node("masa");
        let salida = rack.world_mut().add_node("psu_out");

        let mut psu = GenericDcSupply::generic("PSU0001");
        psu.wire(salida);

        let mut dmm = GenericDmm::generic("DMM0001");
        dmm.wire(Terminal::wired("HI", salida), Terminal::wired("LO", masa));

        let id_psu = rack.add(Box::new(psu));
        let id_dmm = rack.add(Box::new(dmm));

        (rack, id_psu, id_dmm)
    }

    #[test]
    fn cada_instrumento_tiene_su_propia_identidad() {
        let (rack, psu, dmm) = rack_de_demostracion();
        assert!(rack.idn(psu).unwrap().contains("GPS-3003"));
        assert!(rack.idn(dmm).unwrap().contains("GDM-1000"));
    }

    #[test]
    fn los_comandos_llegan_al_instrumento_correcto() {
        let (mut rack, psu, dmm) = rack_de_demostracion();
        assert!(rack.dispatch(psu, "*IDN?").unwrap().contains("GPS-3003"));
        assert!(rack.dispatch(dmm, "*IDN?").unwrap().contains("GDM-1000"));
    }

    #[test]
    fn un_comando_sin_consulta_no_produce_respuesta() {
        let (mut rack, psu, _) = rack_de_demostracion();
        assert_eq!(rack.dispatch(psu, "OUTP ON"), None);
    }

    /// La demostración completa de punta a punta, que es lo que verá quien se
    /// conecte: se programa la fuente por un puerto y se lee el resultado por
    /// otro, con el tiempo de establecimiento de por medio.
    #[test]
    fn se_programa_la_fuente_y_lo_mide_el_multimetro() {
        let (mut rack, psu, dmm) = rack_de_demostracion();

        rack.dispatch(psu, "VOLT 3.3;:OUTP ON");
        rack.run_for(Duration::from_millis(50));

        let lectura: f64 = rack
            .dispatch(dmm, "MEAS:VOLT:DC?")
            .unwrap()
            .parse()
            .unwrap();
        assert!((lectura - 3.3).abs() < 5e-3, "el multímetro leyó {lectura}");
    }

    /// Y el establecimiento se nota, con un matiz que conviene entender: el
    /// multímetro integra hacia delante desde el instante en que se le pide la
    /// medida. Con la integración por defecto de 1 NPLC —20 ms a 50 Hz— la
    /// ventana se come el establecimiento entero y la lectura sale casi
    /// asentada, igual que en el banco real. Para ver el transitorio hay que
    /// acortar la integración, que es exactamente lo que se hace con un
    /// instrumento de verdad.
    #[test]
    fn medir_con_integracion_rapida_deja_ver_el_transitorio() {
        let (mut rack, psu, dmm) = rack_de_demostracion();

        rack.dispatch(dmm, "VOLT:DC:NPLC 0.02"); // 400 µs de apertura
        rack.dispatch(psu, "VOLT 10;:OUTP ON");
        rack.run_for(Duration::from_millis(5)); // la mitad del establecimiento

        let lectura: f64 = rack
            .dispatch(dmm, "MEAS:VOLT:DC?")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (4.0..6.5).contains(&lectura),
            "debería estar a media rampa, no en {lectura}"
        );
    }

    /// Con la integración por defecto, en cambio, la propia ventana de medida
    /// promedia el transitorio y la lectura sale prácticamente asentada.
    #[test]
    fn con_integracion_lenta_la_ventana_promedia_el_transitorio() {
        let (mut rack, psu, dmm) = rack_de_demostracion();

        rack.dispatch(psu, "VOLT 10;:OUTP ON");
        rack.run_for(Duration::from_millis(5));

        let lectura: f64 = rack
            .dispatch(dmm, "MEAS:VOLT:DC?")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            lectura > 9.0,
            "la ventana larga debería promediar: {lectura}"
        );
    }

    #[test]
    fn el_reloj_avanza_para_todos_a_la_vez() {
        let (mut rack, _, _) = rack_de_demostracion();
        rack.run_for(Duration::from_millis(10));
        assert_eq!(rack.now().as_nanos(), 10_000_000);
    }
}
