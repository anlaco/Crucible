//! El reloj de la simulación.
//!
//! El motor no lee nunca la hora del sistema directamente: se la pide al reloj.
//! Esa indirección es lo que permite ejecutar exactamente el mismo escenario en
//! dos modos distintos sin tocar ni una línea de los instrumentos:
//!
//! - [`WallClock`]: sincronizado con el reloj de pared. Es el modo de trabajo,
//!   el que hace que un cliente externo perciba tiempos realistas.
//! - [`VirtualClock`]: el tiempo lo dicta el motor y avanza tan rápido como la
//!   CPU permita. Es el modo de los tests: reproducible al bit y sin esperas.

use std::time::{Duration, Instant};

use crate::SimTime;

/// Fuente de tiempo de la simulación.
///
/// `Send` significa que el tipo puede transferirse a otro hilo. Lo exigimos aquí
/// porque el reloj vivirá en el hilo de simulación, que se crea aparte.
pub trait Clock: Send {
    /// Instante actual de la simulación.
    fn now(&self) -> SimTime;

    /// Periodo entre dos tics consecutivos.
    fn period(&self) -> Duration;

    /// Avanza un tic y devuelve el instante nuevo.
    ///
    /// Un reloj de pared bloquea aquí hasta que llegue el momento; uno virtual
    /// devuelve de inmediato.
    fn tick(&mut self) -> SimTime;
}

/// Reloj virtual: avanza cuando se le dice, sin esperar a nadie.
///
/// Es el reloj por defecto de los tests. Al no depender del reloj del sistema,
/// dos ejecuciones del mismo escenario producen resultados idénticos.
#[derive(Debug, Clone)]
pub struct VirtualClock {
    now: SimTime,
    period: Duration,
}

impl VirtualClock {
    /// Crea un reloj virtual con la frecuencia de tic indicada.
    pub fn new(period: Duration) -> Self {
        assert!(!period.is_zero(), "el periodo del reloj no puede ser cero");
        Self {
            now: SimTime::ZERO,
            period,
        }
    }

    /// Atajo para expresar la frecuencia en hercios en vez de en periodo.
    pub fn from_hz(hz: f64) -> Self {
        assert!(hz > 0.0, "la frecuencia del reloj debe ser positiva");
        Self::new(Duration::from_secs_f64(1.0 / hz))
    }

    /// Salta hacia delante varios tics de golpe.
    ///
    /// Útil para los tests: permite situarse en el segundo 3600 de simulación
    /// sin haber ejecutado tres millones y medio de tics.
    pub fn advance(&mut self, by: Duration) {
        self.now += by;
    }
}

impl Clock for VirtualClock {
    fn now(&self) -> SimTime {
        self.now
    }

    fn period(&self) -> Duration {
        self.period
    }

    fn tick(&mut self) -> SimTime {
        self.now += self.period;
        self.now
    }
}

/// Reloj de pared: cada tic espera a que transcurra el periodo real.
///
/// La planificación es **absoluta**, no incremental: cada tic se calcula como
/// "arranque más N periodos", no como "ahora más un periodo". La diferencia
/// importa mucho. Sumando incrementos, el pequeño retraso de cada iteración se
/// acumula y al cabo de un rato la simulación va notablemente por detrás. Con
/// referencia absoluta, un tic que llega tarde no arrastra al siguiente.
///
/// Es tiempo real *blando*: si el sistema se atasca, el reloj no espera y sigue
/// adelante. No hay garantías duras y no se pretenden.
#[derive(Debug)]
pub struct WallClock {
    /// Momento real en que arrancó la simulación.
    origin: Instant,
    now: SimTime,
    period: Duration,
}

impl WallClock {
    pub fn new(period: Duration) -> Self {
        assert!(!period.is_zero(), "el periodo del reloj no puede ser cero");
        Self {
            origin: Instant::now(),
            now: SimTime::ZERO,
            period,
        }
    }

    pub fn from_hz(hz: f64) -> Self {
        assert!(hz > 0.0, "la frecuencia del reloj debe ser positiva");
        Self::new(Duration::from_secs_f64(1.0 / hz))
    }

    /// Cuánto se ha desviado la simulación del tiempo real.
    ///
    /// Positivo significa que vamos por detrás. Sirve para vigilar si el motor
    /// aguanta la carga y para avisar por la web cuando deja de hacerlo.
    pub fn lag(&self) -> Duration {
        let objetivo = Duration::from_nanos(self.now.as_nanos());
        self.origin.elapsed().saturating_sub(objetivo)
    }
}

impl Clock for WallClock {
    fn now(&self) -> SimTime {
        self.now
    }

    fn period(&self) -> Duration {
        self.period
    }

    fn tick(&mut self) -> SimTime {
        self.now += self.period;

        // Momento real en que este tic debería producirse, medido desde el
        // origen. Nunca desde "ahora": ahí está la diferencia entre acumular
        // deriva y no acumularla.
        let objetivo = Duration::from_nanos(self.now.as_nanos());
        let transcurrido = self.origin.elapsed();

        if objetivo > transcurrido {
            std::thread::sleep(objetivo - transcurrido);
        }
        // Si ya vamos tarde no se duerme: se sigue de inmediato y el retraso
        // queda reflejado en `lag()`.

        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_reloj_virtual_avanza_un_periodo_por_tic() {
        let mut reloj = VirtualClock::from_hz(1000.0); // 1 kHz -> 1 ms

        assert_eq!(reloj.now(), SimTime::ZERO);
        assert_eq!(reloj.tick().as_nanos(), 1_000_000);
        assert_eq!(reloj.tick().as_nanos(), 2_000_000);
    }

    #[test]
    fn el_reloj_virtual_no_acumula_error_de_redondeo() {
        // Mil tics de 1 ms deben dar exactamente un segundo, ni un nanosegundo
        // más ni uno menos. Con tiempo en coma flotante esto no se cumpliría.
        let mut reloj = VirtualClock::from_hz(1000.0);
        for _ in 0..1000 {
            reloj.tick();
        }
        assert_eq!(reloj.now().as_nanos(), 1_000_000_000);
    }

    #[test]
    fn el_reloj_virtual_puede_saltar() {
        let mut reloj = VirtualClock::from_hz(1000.0);
        reloj.advance(Duration::from_secs(3600));
        assert_eq!(reloj.now().as_secs_f64(), 3600.0);
    }

    #[test]
    fn el_reloj_de_pared_espera_de_verdad() {
        // Periodo grande a propósito para que la medida sea fiable incluso en
        // una máquina cargada. Solo comprobamos que no devuelve al instante.
        let mut reloj = WallClock::new(Duration::from_millis(20));
        let antes = Instant::now();
        reloj.tick();
        let real = antes.elapsed();

        assert_eq!(reloj.now().as_nanos(), 20_000_000);
        assert!(real >= Duration::from_millis(18), "apenas esperó: {real:?}");
    }

    #[test]
    fn el_reloj_de_pared_no_acumula_deriva() {
        // Cinco tics de 5 ms deben tardar en torno a 25 ms en total, no más.
        // Si la planificación fuese incremental, cada iteración sumaría su
        // propio retraso y el total se dispararía.
        let mut reloj = WallClock::new(Duration::from_millis(5));
        let antes = Instant::now();
        for _ in 0..5 {
            reloj.tick();
        }
        let real = antes.elapsed();

        assert_eq!(reloj.now().as_nanos(), 25_000_000);
        assert!(
            real < Duration::from_millis(60),
            "deriva excesiva: {real:?}"
        );
    }

    /// El motor guarda el reloj tras un `Box<dyn Clock>`, así que ambos tipos
    /// tienen que ser intercambiables sin que el código de alrededor cambie.
    #[test]
    fn ambos_relojes_son_intercambiables() {
        let mut relojes: Vec<Box<dyn Clock>> = vec![
            Box::new(VirtualClock::from_hz(1000.0)),
            Box::new(WallClock::from_hz(1000.0)),
        ];

        for reloj in &mut relojes {
            assert_eq!(reloj.tick().as_nanos(), 1_000_000);
        }
    }
}
