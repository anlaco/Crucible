//! Representación del tiempo de simulación.

use std::ops::{Add, AddAssign, Sub};
use std::time::Duration;

/// Un instante de la simulación, medido en nanosegundos desde el arranque.
///
/// Es un "newtype": una `struct` que envuelve un único `u64`. En tiempo de
/// ejecución ocupa exactamente lo mismo que un `u64`, pero el compilador impide
/// confundirlo con cualquier otro número. Así es imposible sumar por error un
/// instante y un número de muestras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimTime {
    // Sin `pub`: el campo es privado. Nadie fuera de este módulo puede tocarlo
    // directamente, solo a través de los métodos de abajo. Eso nos deja libertad
    // para cambiar la representación interna en el futuro sin romper a nadie.
    nanos: u64,
}

impl SimTime {
    /// El instante cero: el arranque de la simulación.
    ///
    /// Una constante asociada. Se usa como `SimTime::ZERO`.
    pub const ZERO: SimTime = SimTime { nanos: 0 };

    /// Construye un instante a partir de nanosegundos.
    ///
    /// `const fn` significa que el compilador puede evaluarla en tiempo de
    /// compilación, así que sirve para inicializar constantes.
    pub const fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    /// Los nanosegundos transcurridos desde el arranque.
    ///
    /// Fíjate en que recibe `self` por valor, no `&self`. Como `SimTime` es
    /// `Copy` (lo pedimos en el `derive`), se comporta como un entero: se copia
    /// en vez de moverse, y no hay que preocuparse de préstamos ni de duración
    /// de la referencia.
    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    /// Construye un instante a partir de segundos.
    ///
    /// Cómodo para escribir tests y para leer configuración.
    pub fn from_secs_f64(secs: f64) -> Self {
        debug_assert!(secs >= 0.0, "SimTime no puede ser negativo: {secs}");
        Self {
            nanos: (secs * 1e9).round() as u64,
        }
    }

    /// El instante expresado en segundos.
    ///
    /// Esta es la forma en que las señales recibirán el tiempo: la física se
    /// escribe en segundos, el motor la guarda en nanosegundos.
    pub fn as_secs_f64(self) -> f64 {
        self.nanos as f64 * 1e-9
    }
}

// A partir de aquí, sobrecarga de operadores. En Rust `+` y `-` no son magia del
// lenguaje: son los traits `Add` y `Sub`. Implementarlos para tu tipo es lo que
// permite escribir `t + dt` en vez de `t.add(dt)`.

/// `SimTime + Duration = SimTime`. Avanzar en el tiempo.
impl Add<Duration> for SimTime {
    // `Output` es un tipo asociado: qué devuelve la operación. Un instante más
    // un intervalo sigue siendo un instante.
    type Output = SimTime;

    fn add(self, rhs: Duration) -> SimTime {
        // `Duration::as_nanos()` devuelve `u128` porque `Duration` puede
        // representar mucho más que 584 años. Convertimos y fallamos
        // ruidosamente si alguien pretende avanzar un intervalo absurdo: eso es
        // un error del programador, no una condición que tolerar en silencio.
        let d = u64::try_from(rhs.as_nanos()).expect("duración fuera de rango");
        SimTime {
            nanos: self.nanos + d,
        }
    }
}

/// `t += dt`. Lo mismo, pero modificando en sitio.
impl AddAssign<Duration> for SimTime {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

/// `SimTime - SimTime = Duration`. La diferencia entre dos instantes es un
/// intervalo, no un instante. El sistema de tipos nos obliga a no confundirlos,
/// igual que en física no se suman magnitudes de distinta dimensión.
impl Sub for SimTime {
    type Output = Duration;

    fn sub(self, rhs: SimTime) -> Duration {
        // Satura en cero si se restan al revés, en vez de desbordar.
        Duration::from_nanos(self.nanos.saturating_sub(rhs.nanos))
    }
}

// `#[cfg(test)]` significa "compila esto solo cuando se ejecuten los tests".
// En el binario final no existe. En Rust los tests unitarios viven junto al
// código que prueban, no en una carpeta aparte.
#[cfg(test)]
mod tests {
    // `super` es el módulo padre, o sea `time`. Importa todo lo público de él.
    use super::*;

    #[test]
    fn el_cero_es_cero() {
        assert_eq!(SimTime::ZERO.as_nanos(), 0);
    }

    #[test]
    fn conversion_segundos_ida_y_vuelta() {
        let t = SimTime::from_secs_f64(1.5);
        assert_eq!(t.as_nanos(), 1_500_000_000);
        assert_eq!(t.as_secs_f64(), 1.5);
    }

    #[test]
    fn avanzar_en_el_tiempo() {
        let t = SimTime::ZERO + Duration::from_millis(1);
        assert_eq!(t.as_nanos(), 1_000_000);

        let mut t = SimTime::ZERO;
        t += Duration::from_micros(250);
        assert_eq!(t.as_nanos(), 250_000);
    }

    #[test]
    fn diferencia_de_instantes_es_un_intervalo() {
        let a = SimTime::from_nanos(1_000);
        let b = SimTime::from_nanos(4_000);
        assert_eq!(b - a, Duration::from_nanos(3_000));
        // Restar al revés satura en cero en lugar de desbordar.
        assert_eq!(a - b, Duration::ZERO);
    }

    #[test]
    fn los_instantes_se_ordenan() {
        assert!(SimTime::from_nanos(1) < SimTime::from_nanos(2));
    }

    /// La razón de ser de los nanosegundos enteros: a 1 GS/s dos muestras
    /// consecutivas distan 1 ns, y deben seguir siendo distinguibles aunque la
    /// simulación lleve una hora corriendo.
    #[test]
    fn resolucion_de_un_nanosegundo_tras_una_hora() {
        let una_hora = SimTime::from_nanos(3_600_000_000_000);
        let siguiente_muestra = una_hora + Duration::from_nanos(1);
        assert_ne!(una_hora, siguiente_muestra);
        assert_eq!(siguiente_muestra - una_hora, Duration::from_nanos(1));
    }
}
