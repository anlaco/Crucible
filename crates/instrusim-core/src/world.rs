//! El mundo: el estado físico compartido del rack.
//!
//! Es la tercera invariante del proyecto, y la que separa un simulador de un
//! gemelo digital: **la capa SCPI nunca calcula un valor**. Cuando llega
//! `MEAS:VOLT:DC?`, el protocolo pregunta al modelo del multímetro, el modelo
//! mira qué hay en el nodo al que están conectados sus bornes, y el nodo
//! devuelve la señal que alguien puso ahí.
//!
//! Hoy quien pone esa señal es el escenario, escrita a mano en un fichero. En
//! la fase 6 la pondrá el análisis nodal a partir de lo que apliquen las
//! fuentes. Los instrumentos no se enterarán del cambio, porque preguntan
//! exactamente igual en los dos casos. Por eso el mundo existe desde el primer
//! día aunque todavía no resuelva ningún circuito: es la costura por la que
//! entrará la física, y tiene que estar cosida antes de que haya nada que coser.

use crate::SimTime;
use crate::signal::Signal;
use crate::trigger::TriggerBus;

/// Identificador de un nodo eléctrico del rack.
///
/// Es un índice dentro del vector de nodos del mundo, envuelto para que no se
/// pueda confundir con cualquier otro entero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u32);

impl NodeId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

/// Un punto de conexión eléctrica del rack: el equivalente a un borne de una
/// regleta o a un nodo de un esquema.
#[derive(Debug, Clone)]
pub struct Node {
    /// Nombre legible, para la vista web y los mensajes de error.
    pub name: String,
    /// Lo que hay en este nodo, en función del tiempo.
    pub potential: Signal,
}

/// Un borne de un instrumento: el punto por donde se conecta al mundo.
///
/// `Option<NodeId>` es el sustituto de `null`: o hay un nodo conectado
/// (`Some(id)`) o el borne está al aire (`None`). El compilador obliga a tratar
/// el caso "al aire" en cada uso, así que es imposible olvidarse de él, que es
/// exactamente lo que pasa con un cable suelto en un banco real.
#[derive(Debug, Clone)]
pub struct Terminal {
    pub name: String,
    pub node: Option<NodeId>,
}

impl Terminal {
    /// Un borne sin conectar.
    pub fn floating(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            node: None,
        }
    }

    /// Un borne cableado a un nodo.
    pub fn wired(name: impl Into<String>, node: NodeId) -> Self {
        Self {
            name: name.into(),
            node: Some(node),
        }
    }
}

/// El estado físico compartido de la simulación.
#[derive(Debug, Default)]
pub struct World {
    now: SimTime,
    nodes: Vec<Node>,
    /// Las líneas de disparo del rack.
    pub triggers: TriggerBus,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Instante actual de la simulación.
    ///
    /// Lo fija el motor en cada tic a partir del reloj. Los instrumentos lo
    /// consultan aquí en vez de mirar la hora del sistema, que es lo que hace
    /// posible el modo de tiempo virtual.
    pub fn now(&self) -> SimTime {
        self.now
    }

    /// Fija el instante actual.
    ///
    /// Normalmente solo lo llama el motor al principio de cada tic. Es público
    /// para poder situar el mundo en un instante concreto desde un test o desde
    /// un escenario, sin tener que ejecutar los tics intermedios.
    pub fn set_now(&mut self, t: SimTime) {
        self.now = t;
    }

    /// Crea un nodo y devuelve su identificador.
    pub fn add_node(&mut self, name: impl Into<String>) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node {
            name: name.into(),
            potential: Signal::ZERO,
        });
        id
    }

    /// Crea un nodo con una señal ya puesta.
    pub fn add_node_with(&mut self, name: impl Into<String>, potential: Signal) -> NodeId {
        let id = self.add_node(name);
        self.nodes[id.index()].potential = potential;
        id
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    /// Busca un nodo por su nombre. Devuelve `None` si no existe.
    pub fn node_by_name(&self, name: &str) -> Option<NodeId> {
        self.nodes
            .iter()
            .position(|n| n.name == name)
            .map(|i| NodeId(i as u32))
    }

    /// Impone una señal en un nodo. Es lo que hace una fuente al aplicar
    /// tensión, o el escenario al preparar un valor de prueba.
    pub fn drive(&mut self, id: NodeId, potential: Signal) {
        self.nodes[id.index()].potential = potential;
    }

    /// La señal presente en un nodo.
    pub fn potential(&self, id: NodeId) -> &Signal {
        &self.nodes[id.index()].potential
    }

    /// El valor en un nodo en un instante concreto.
    pub fn potential_at(&self, id: NodeId, t: SimTime) -> f64 {
        self.nodes[id.index()].potential.eval(t)
    }

    /// El valor en un nodo *ahora*. El atajo que usarán los instrumentos.
    pub fn potential_now(&self, id: NodeId) -> f64 {
        self.potential_at(id, self.now)
    }

    /// La diferencia de potencial entre dos bornes en un instante cualquiera.
    ///
    /// Hace falta para medir alterna: un multímetro no lee el valor eficaz de
    /// un golpe, integra la señal a lo largo de su tiempo de apertura. Como los
    /// nodos guardan señales evaluables, el instrumento puede muestrear esa
    /// ventana a la resolución que necesite sin que el motor corra más deprisa.
    pub fn differential_at(&self, high: &Terminal, low: &Terminal, t: SimTime) -> f64 {
        let v = |term: &Terminal| match term.node {
            Some(id) => self.potential_at(id, t),
            None => 0.0,
        };
        v(high) - v(low)
    }

    /// La diferencia de potencial entre dos bornes, en el instante actual.
    ///
    /// Un borne al aire cuenta como cero voltios. Es una simplificación
    /// deliberada mientras no haya análisis nodal: un multímetro real con una
    /// punta suelta capta ruido y deriva, y eso llegará con la física. Que la
    /// simplificación esté aquí y no repartida por cada instrumento es
    /// precisamente el motivo de que este método exista.
    pub fn differential_now(&self, high: &Terminal, low: &Terminal) -> f64 {
        let v = |term: &Terminal| match term.node {
            Some(id) => self.potential_now(id),
            None => 0.0,
        };
        v(high) - v(low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn un_mundo_recien_creado_esta_vacio_y_en_el_instante_cero() {
        let mundo = World::new();
        assert_eq!(mundo.node_count(), 0);
        assert_eq!(mundo.now(), SimTime::ZERO);
    }

    #[test]
    fn un_nodo_nuevo_esta_a_cero_voltios() {
        let mut mundo = World::new();
        let n = mundo.add_node("salida");
        assert_eq!(mundo.potential_now(n), 0.0);
    }

    #[test]
    fn los_nodos_se_pueden_buscar_por_nombre() {
        let mut mundo = World::new();
        let n = mundo.add_node("dut_vcc");
        mundo.add_node("masa");

        assert_eq!(mundo.node_by_name("dut_vcc"), Some(n));
        assert_eq!(mundo.node_by_name("inexistente"), None);
    }

    #[test]
    fn aplicar_una_senal_a_un_nodo_cambia_lo_que_se_mide_en_el() {
        let mut mundo = World::new();
        let n = mundo.add_node("salida");

        mundo.drive(n, Signal::Constant(5.0));
        assert_eq!(mundo.potential_now(n), 5.0);
    }

    /// La propiedad que justifica todo el módulo: el nodo guarda una señal, no
    /// un número, así que el valor medido depende del instante sin que nadie
    /// tenga que ir refrescando nada.
    #[test]
    fn el_valor_de_un_nodo_depende_del_instante_en_que_se_mire() {
        let mut mundo = World::new();
        let n = mundo.add_node_with("generador", Signal::sine(1.0, 1.0));

        assert_eq!(mundo.potential_at(n, SimTime::ZERO), 0.0);
        assert!((mundo.potential_at(n, SimTime::from_secs_f64(0.25)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn avanzar_el_tiempo_cambia_lo_que_se_mide_ahora() {
        let mut mundo = World::new();
        let n = mundo.add_node_with("generador", Signal::sine(1.0, 1.0));

        assert_eq!(mundo.potential_now(n), 0.0);

        mundo.set_now(SimTime::from_secs_f64(0.25));
        assert!((mundo.potential_now(n) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn la_medida_diferencial_resta_los_dos_bornes() {
        let mut mundo = World::new();
        let alto = mundo.add_node_with("hi", Signal::Constant(5.0));
        let bajo = mundo.add_node_with("lo", Signal::Constant(2.0));

        let hi = Terminal::wired("HI", alto);
        let lo = Terminal::wired("LO", bajo);

        assert_eq!(mundo.differential_now(&hi, &lo), 3.0);
    }

    #[test]
    fn un_borne_al_aire_cuenta_como_cero() {
        let mut mundo = World::new();
        let alto = mundo.add_node_with("hi", Signal::Constant(5.0));

        let hi = Terminal::wired("HI", alto);
        let lo = Terminal::floating("LO");

        assert_eq!(mundo.differential_now(&hi, &lo), 5.0);
    }

    /// Los triggers viven en el mundo, así que dos instrumentos cualesquiera
    /// pueden sincronizarse a través de él sin conocerse entre sí.
    #[test]
    fn el_mundo_transporta_los_disparos() {
        use crate::trigger::LineId;

        let mut mundo = World::new();
        mundo.set_now(SimTime::from_secs_f64(1.0));

        // Un instrumento dispara...
        let ahora = mundo.now();
        mundo.triggers.pulse(LineId(1), ahora);

        // ...y otro lo recoge.
        let eventos = mundo.triggers.take_until(LineId(1), mundo.now());
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].at, SimTime::from_secs_f64(1.0));
    }

    /// El escenario completo en miniatura: una fuente que arranca con una rampa
    /// de establecimiento y un multímetro que la mide en distintos instantes.
    /// Es lo que hará el motor de verdad, pero sin motor todavía.
    #[test]
    fn escenario_minimo_fuente_y_multimetro() {
        let mut mundo = World::new();

        let masa = mundo.add_node("masa");
        let salida = mundo.add_node_with(
            "fuente_out",
            Signal::Ramp {
                from: 0.0,
                to: 5.0,
                start: SimTime::ZERO,
                duration: Duration::from_millis(10),
            }
            .with_noise(50e-6, 1),
        );

        let hi = Terminal::wired("HI", salida);
        let lo = Terminal::wired("LO", masa);

        // Nada más arrancar, la fuente aún no ha establecido.
        mundo.set_now(SimTime::ZERO);
        assert!(mundo.differential_now(&hi, &lo).abs() < 1e-3);

        // A mitad de la rampa, la mitad de la tensión.
        mundo.set_now(SimTime::from_secs_f64(0.005));
        assert!((mundo.differential_now(&hi, &lo) - 2.5).abs() < 1e-3);

        // Ya establecida, cinco voltios con su ruidillo de 50 µV.
        mundo.set_now(SimTime::from_secs_f64(1.0));
        let medida = mundo.differential_now(&hi, &lo);
        assert!((medida - 5.0).abs() < 1e-3, "medida: {medida}");
        assert_ne!(medida, 5.0, "debería tener ruido, no ser exacto");
    }
}
