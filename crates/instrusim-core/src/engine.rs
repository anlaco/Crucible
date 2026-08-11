//! El motor: quien junta reloj, mundo y participantes y los hace avanzar.
//!
//! Un único hilo posee el mundo y es el único que lo modifica. Los servidores de
//! red que vendrán en la fase 2 no tocarán nada directamente: le mandarán
//! comandos por un canal y recibirán la respuesta de vuelta. No hay estado
//! compartido entre hilos, así que no hay cerrojos ni condiciones de carrera
//! posibles. Y bajo reloj virtual, la ejecución es reproducible al bit.

use std::time::Duration;

use crate::SimTime;
use crate::clock::Clock;
use crate::world::World;

/// Algo que participa en la simulación y necesita reaccionar al paso del tiempo.
///
/// Los instrumentos lo implementarán en el crate `instrusim-model`. Está aquí,
/// deliberadamente mínimo, para que el núcleo no dependa de nadie.
pub trait Stepper: Send {
    /// Avanza el estado interno un paso de reloj.
    ///
    /// Es donde un multímetro decide si ya ha terminado su integración, donde
    /// una fuente avanza su rampa de establecimiento y donde cualquiera consulta
    /// sus líneas de disparo.
    fn step(&mut self, world: &mut World, dt: Duration);
}

/// El bucle de simulación.
pub struct Engine {
    world: World,
    /// `Box<dyn Clock>` es despacho dinámico: el motor no sabe si dentro hay un
    /// reloj de pared o uno virtual, solo que cumple el contrato. Es lo mismo
    /// que declarar en Java una variable con el tipo de la interfaz.
    clock: Box<dyn Clock>,
    steppers: Vec<Box<dyn Stepper>>,
}

impl Engine {
    pub fn new(clock: Box<dyn Clock>) -> Self {
        let mut world = World::new();
        world.set_now(clock.now());
        Self {
            world,
            clock,
            steppers: Vec::new(),
        }
    }

    /// Incorpora un participante a la simulación.
    pub fn add(&mut self, stepper: Box<dyn Stepper>) {
        self.steppers.push(stepper);
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Acceso mutable al mundo, para que el escenario lo prepare antes de
    /// arrancar: crear nodos, cablear bornes, imponer señales.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn now(&self) -> SimTime {
        self.world.now()
    }

    /// Ejecuta un tic completo.
    ///
    /// Con reloj de pared, esta llamada bloquea hasta que llega el momento; con
    /// reloj virtual vuelve de inmediato. Los participantes no notan diferencia.
    pub fn tick(&mut self) -> SimTime {
        let ahora = self.clock.tick();
        let dt = self.clock.period();

        // El instante se fija **antes** de que nadie se ejecute, para que todos
        // los participantes vean exactamente el mismo "ahora" dentro del tic.
        // Si cada uno leyese el reloj por su cuenta, dos instrumentos que
        // midiesen "a la vez" obtendrían instantes distintos y el rack dejaría
        // de ser coherente.
        self.world.set_now(ahora);

        for stepper in self.steppers.iter_mut() {
            stepper.step(&mut self.world, dt);
        }

        ahora
    }

    /// Ejecuta un número fijo de tics.
    pub fn run_ticks(&mut self, n: usize) {
        for _ in 0..n {
            self.tick();
        }
    }

    /// Ejecuta tics hasta cubrir el intervalo pedido.
    pub fn run_for(&mut self, duration: Duration) {
        let hasta = self.world.now() + duration;
        while self.world.now() < hasta {
            self.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::clock::VirtualClock;
    use crate::signal::Signal;
    use crate::trigger::{Edge, LineId};
    use crate::world::{NodeId, Terminal};

    /// Participante de prueba: cuenta cuántas veces le han hecho avanzar.
    ///
    /// El contador va tras un `Arc<AtomicUsize>` porque al añadir el
    /// participante al motor se le cede la propiedad y ya no se puede volver a
    /// mirar dentro. `Arc` es un puntero contado por referencias: el test se
    /// queda con una copia del puntero y puede seguir leyendo el contador
    /// mientras el motor es dueño del participante.
    struct Contador {
        pasos: Arc<AtomicUsize>,
    }

    impl Stepper for Contador {
        fn step(&mut self, _world: &mut World, _dt: Duration) {
            self.pasos.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Una fuente rudimentaria: al llegar su instante, impone tensión en su
    /// nodo y avisa por una línea de disparo.
    struct FuenteConAviso {
        nodo: NodeId,
        arranca_en: SimTime,
        tension: f64,
        linea: LineId,
        ya_arrancada: bool,
    }

    impl Stepper for FuenteConAviso {
        fn step(&mut self, world: &mut World, _dt: Duration) {
            if !self.ya_arrancada && world.now() >= self.arranca_en {
                world.drive(self.nodo, Signal::Constant(self.tension));
                let ahora = world.now();
                world.triggers.emit(self.linea, ahora, Edge::Rising);
                self.ya_arrancada = true;
            }
        }
    }

    /// Un medidor que solo mide cuando le disparan, y guarda la lectura.
    struct MedidorDisparado {
        hi: Terminal,
        lo: Terminal,
        linea: LineId,
        lecturas: Vec<(SimTime, f64)>,
    }

    impl Stepper for MedidorDisparado {
        fn step(&mut self, world: &mut World, _dt: Duration) {
            let ahora = world.now();
            let disparos = world.triggers.take_until(self.linea, ahora);
            for _ in disparos {
                let v = world.differential_now(&self.hi, &self.lo);
                self.lecturas.push((ahora, v));
            }
        }
    }

    fn motor_virtual(hz: f64) -> Engine {
        Engine::new(Box::new(VirtualClock::from_hz(hz)))
    }

    #[test]
    fn el_motor_avanza_el_reloj_del_mundo() {
        let mut motor = motor_virtual(1000.0); // 1 kHz

        assert_eq!(motor.now(), SimTime::ZERO);
        motor.tick();
        assert_eq!(motor.now().as_nanos(), 1_000_000);
        motor.run_ticks(9);
        assert_eq!(motor.now().as_nanos(), 10_000_000);
    }

    #[test]
    fn todos_los_participantes_avanzan_en_cada_tic() {
        let mut motor = motor_virtual(1000.0);

        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));
        motor.add(Box::new(Contador {
            pasos: Arc::clone(&a),
        }));
        motor.add(Box::new(Contador {
            pasos: Arc::clone(&b),
        }));

        motor.run_ticks(5);

        assert_eq!(a.load(Ordering::Relaxed), 5);
        assert_eq!(b.load(Ordering::Relaxed), 5);
        assert_eq!(motor.now().as_nanos(), 5_000_000);
    }

    #[test]
    fn run_for_cubre_el_intervalo_pedido() {
        let mut motor = motor_virtual(1000.0); // tics de 1 ms
        motor.run_for(Duration::from_millis(10));
        assert_eq!(motor.now().as_nanos(), 10_000_000);
    }

    /// El hito de coordinación de la fase 1: dos instrumentos que no se conocen
    /// entre sí se sincronizan a través del mundo. La fuente arranca a los 5 ms,
    /// avisa por la línea 1, y el medidor —que solo mide cuando le disparan—
    /// captura la tensión en ese instante exacto.
    #[test]
    fn dos_instrumentos_se_sincronizan_por_el_bus_de_disparo() {
        let mut motor = motor_virtual(1000.0);

        let masa = motor.world_mut().add_node("masa");
        let salida = motor.world_mut().add_node("fuente_out");

        let linea = LineId(1);

        motor.add(Box::new(FuenteConAviso {
            nodo: salida,
            arranca_en: SimTime::from_secs_f64(0.005),
            tension: 3.3,
            linea,
            ya_arrancada: false,
        }));

        // El medidor se añade después de la fuente, así que dentro del mismo
        // tic ve el disparo que la fuente acaba de emitir. El orden importa y
        // está bajo control: los participantes se ejecutan en el orden en que
        // se añadieron.
        motor.add(Box::new(MedidorDisparado {
            hi: Terminal::wired("HI", salida),
            lo: Terminal::wired("LO", masa),
            linea,
            lecturas: Vec::new(),
        }));

        motor.run_for(Duration::from_millis(20));

        // La comprobación observable desde fuera: la fuente dejó su tensión
        // puesta en el nodo, y el disparo fue consumido por el medidor en vez
        // de quedarse pendiente.
        assert_eq!(motor.world().potential_now(salida), 3.3);
        assert!(
            motor.world().triggers.is_empty(),
            "el medidor debería haber consumido el disparo"
        );
    }

    /// La reproducibilidad que justifica el reloj virtual: dos ejecuciones
    /// idénticas producen exactamente los mismos valores, incluido el ruido.
    #[test]
    fn el_reloj_virtual_hace_la_simulacion_reproducible() {
        fn ejecutar() -> Vec<f64> {
            let mut motor = motor_virtual(1000.0);
            let n = motor
                .world_mut()
                .add_node_with("ruidoso", Signal::Constant(5.0).with_noise(1e-3, 99));

            let mut lecturas = Vec::new();
            for _ in 0..10 {
                motor.tick();
                lecturas.push(motor.world().potential_now(n));
            }
            lecturas
        }

        assert_eq!(ejecutar(), ejecutar());
    }
}
