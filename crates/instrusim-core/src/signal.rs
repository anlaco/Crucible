//! Señales: funciones del tiempo que se pueden evaluar a cualquier resolución.
//!
//! Es la pieza que decide el techo del proyecto. Un simulador ingenuo guarda un
//! número por nodo y lo refresca al ritmo del reloj; con eso jamás podría haber
//! osciloscopios, porque muestrear a 1 GS/s exigiría correr el motor a 1 GHz.
//!
//! Aquí un nodo no guarda un valor sino **una función del tiempo**. El generador
//! deja dicho "senoide de 1 MHz"; el osciloscopio pide diez mil muestras
//! separadas un nanosegundo y el motor las *evalúa*. El reloj de 1 kHz solo
//! gobierna los cambios de configuración y los transitorios lentos.
//!
//! Y hay una segunda razón, menos evidente, que es la que hará posible el rack
//! completo: cuando llegue el análisis nodal, la red será lineal, así que por
//! superposición el potencial de un nodo es una combinación lineal de las
//! señales de las fuentes. El solver producirá ganancias, y el resultado se
//! expresa con [`Signal::Sum`] y [`Signal::Scaled`] sobre las señales
//! originales. Es decir: **física compartida y fidelidad en alta frecuencia sin
//! resolver el circuito en alta frecuencia**.

use std::f64::consts::TAU;
use std::sync::Arc;
use std::time::Duration;

use crate::SimTime;

/// Una magnitud eléctrica en función del tiempo.
///
/// Un `enum` de Rust no es una lista de constantes como el de Java o C: cada
/// variante puede llevar sus propios datos. Es lo que en teoría de tipos se
/// llama un tipo suma, y en Java se aproximaría con una jerarquía de clases
/// selladas. La diferencia práctica es que el compilador obliga a tratar todos
/// los casos: si mañana añadimos una variante, todos los `match` que se hayan
/// olvidado de ella dejan de compilar.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// Valor fijo. El caso más común con diferencia: una fuente en continua,
    /// un nodo a masa, una entrada desconectada.
    Constant(f64),

    /// Senoide. `phase` va en radianes.
    Sine {
        amplitude: f64,
        frequency: f64,
        phase: f64,
        offset: f64,
    },

    /// Rampa lineal entre dos valores. Antes de `start` vale `from`, después de
    /// `start + duration` vale `to`.
    ///
    /// Modela el tiempo de establecimiento de una fuente: no salta de 0 a 5 V,
    /// tarda unos milisegundos. Es el tipo de detalle que separa un simulador
    /// creíble de uno que no lo es.
    Ramp {
        from: f64,
        to: f64,
        start: SimTime,
        duration: Duration,
    },

    /// Onda cuadrada. `duty` es la fracción de periodo en nivel alto, de 0 a 1.
    Square {
        amplitude: f64,
        frequency: f64,
        duty: f64,
        offset: f64,
    },

    /// Un único pulso, para disparos y estímulos.
    Pulse {
        amplitude: f64,
        start: SimTime,
        width: Duration,
        baseline: f64,
    },

    /// Ruido gaussiano de valor eficaz `rms`.
    ///
    /// Es determinista: el mismo `seed` y el mismo instante dan siempre el mismo
    /// valor. No es un generador con estado interno del que se van sacando
    /// muestras, sino una **función pura del tiempo**, igual que las demás. Eso
    /// es imprescindible: `eval` tiene que poder llamarse en cualquier orden y
    /// en cualquier instante, y dar siempre lo mismo. Sin esta propiedad no
    /// habría reproducibilidad y los tests serían inservibles.
    Noise { rms: f64, seed: u64 },

    /// Suma de varias señales. Una fuente con ruido encima, dos fuentes en
    /// serie, el resultado de una superposición.
    Sum(Vec<Signal>),

    /// Una señal multiplicada por una ganancia.
    ///
    /// `Box` es un puntero al montón. Hace falta porque una variante no puede
    /// contener directamente su propio tipo: el compilador no sabría calcular
    /// su tamaño. El `Box` tiene tamaño fijo y rompe la recursión.
    Scaled { inner: Box<Signal>, gain: f64 },

    /// Una forma de onda muestreada, con interpolación lineal entre muestras.
    ///
    /// `Arc` es un puntero contado por referencias, seguro entre hilos. Permite
    /// que varios nodos compartan la misma tabla de un millón de muestras sin
    /// duplicarla. Es lo más parecido a una referencia de Java, pero explícito.
    Sampled(Arc<Waveform>),
}

impl Default for Signal {
    /// Un nodo del que nadie ha dicho nada está a cero voltios.
    fn default() -> Self {
        Signal::Constant(0.0)
    }
}

impl Signal {
    /// Atajo para el caso más frecuente.
    pub const ZERO: Signal = Signal::Constant(0.0);

    /// Construye una senoide sin desfase ni continua.
    pub fn sine(amplitude: f64, frequency: f64) -> Self {
        Signal::Sine { amplitude, frequency, phase: 0.0, offset: 0.0 }
    }

    /// Añade ruido gaussiano a esta señal.
    ///
    /// Consume `self` y devuelve la señal compuesta, para poder encadenar:
    /// `Signal::Constant(5.0).with_noise(50e-6, 42)`.
    pub fn with_noise(self, rms: f64, seed: u64) -> Self {
        Signal::Sum(vec![self, Signal::Noise { rms, seed }])
    }

    /// Multiplica la señal por una ganancia.
    pub fn scaled(self, gain: f64) -> Self {
        Signal::Scaled { inner: Box::new(self), gain }
    }

    /// Valor de la señal en un instante dado.
    ///
    /// Este es el corazón del motor. Puede llamarse en cualquier instante y en
    /// cualquier orden: no hay estado que avanzar.
    pub fn eval(&self, t: SimTime) -> f64 {
        // `match` es como el `switch` de Java pero exhaustivo: el compilador
        // exige cubrir todas las variantes. Además desestructura los datos que
        // cada variante lleva dentro, que es lo que permite escribir
        // `Signal::Sine { amplitude, .. }` y tener ya la variable disponible.
        match self {
            Signal::Constant(v) => *v,

            Signal::Sine { amplitude, frequency, phase, offset } => {
                offset + amplitude * (TAU * fase_normalizada(*frequency, t) + phase).sin()
            }

            Signal::Ramp { from, to, start, duration } => {
                if t <= *start {
                    *from
                } else {
                    let transcurrido = (t - *start).as_secs_f64();
                    let total = duration.as_secs_f64();
                    if transcurrido >= total {
                        *to
                    } else {
                        from + (to - from) * (transcurrido / total)
                    }
                }
            }

            Signal::Square { amplitude, frequency, duty, offset } => {
                let fase = fase_normalizada(*frequency, t);
                if fase < *duty { offset + amplitude } else { offset - amplitude }
            }

            Signal::Pulse { amplitude, start, width, baseline } => {
                let fin = *start + *width;
                if t >= *start && t < fin { *amplitude } else { *baseline }
            }

            Signal::Noise { rms, seed } => rms * gaussiano(*seed, t.as_nanos()),

            Signal::Sum(partes) => partes.iter().map(|s| s.eval(t)).sum(),

            Signal::Scaled { inner, gain } => inner.eval(t) * gain,

            Signal::Sampled(w) => w.eval(t),
        }
    }

    /// Evalúa un bloque de muestras equiespaciadas, empezando en `start`.
    ///
    /// Es la operación que hace viable una captura de osciloscopio: se pide un
    /// buffer entero de una vez en lugar de llamar a `eval` diez mil veces desde
    /// fuera. La aritmética del tiempo se hace en enteros para que la muestra
    /// número diez mil caiga exactamente donde debe, sin deriva acumulada.
    pub fn eval_block(&self, start: SimTime, step: Duration, out: &mut [f64]) {
        let paso_ns = u64::try_from(step.as_nanos()).expect("paso fuera de rango");
        let inicio_ns = start.as_nanos();

        for (i, muestra) in out.iter_mut().enumerate() {
            let t = SimTime::from_nanos(inicio_ns + i as u64 * paso_ns);
            *muestra = self.eval(t);
        }
    }
}

/// Una forma de onda muestreada en el tiempo.
#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    /// Instante de la primera muestra.
    pub start: SimTime,
    /// Separación entre muestras consecutivas.
    pub step: Duration,
    pub samples: Vec<f64>,
}

impl Waveform {
    pub fn new(start: SimTime, step: Duration, samples: Vec<f64>) -> Self {
        assert!(!step.is_zero(), "el paso de muestreo no puede ser cero");
        assert!(!samples.is_empty(), "una forma de onda necesita muestras");
        Self { start, step, samples }
    }

    /// Valor interpolado linealmente. Fuera del rango se mantiene el extremo.
    pub fn eval(&self, t: SimTime) -> f64 {
        if t <= self.start {
            return self.samples[0];
        }

        let paso = self.step.as_secs_f64();
        let posicion = (t - self.start).as_secs_f64() / paso;
        let i = posicion.floor() as usize;

        if i + 1 >= self.samples.len() {
            return *self.samples.last().unwrap();
        }

        let fraccion = posicion - i as f64;
        self.samples[i] * (1.0 - fraccion) + self.samples[i + 1] * fraccion
    }
}

// --- Reducción de fase ----------------------------------------------------

/// Posición dentro del periodo, entre 0 y 1, para una señal periódica.
///
/// Parece un simple `(f * t).fract()`, y de hecho eso es lo que hace todo el
/// mundo. Pero es incorrecto para tiempos largos: con f = 1 MHz y t = 3600 s el
/// producto vale 3,6·10⁹ ciclos, y ahí un `f64` ya solo resuelve unos 5·10⁻⁷
/// ciclos. Es decir, tras una hora de simulación la senoide tendría un error de
/// fase de microradianes, suficiente para estropear una captura de osciloscopio.
///
/// La solución es separar el instante en segundos enteros y resto, ambos
/// obtenidos con aritmética entera exacta, y reducir cada término por separado.
/// El término de los segundos enteros suele cancelarse por completo, y el del
/// resto nunca llega a ser grande, así que la precisión no se degrada por muy
/// larga que sea la simulación.
fn fase_normalizada(frequency: f64, t: SimTime) -> f64 {
    const MIL_MILLONES: u64 = 1_000_000_000;

    let nanos = t.as_nanos();
    let segundos = (nanos / MIL_MILLONES) as f64;
    let resto = (nanos % MIL_MILLONES) as f64 * 1e-9;

    let ciclos = (frequency * segundos).fract() + frequency * resto;
    let fase = ciclos.fract();

    // `fract` conserva el signo, y una frecuencia negativa es legítima.
    if fase < 0.0 { fase + 1.0 } else { fase }
}

// --- Ruido determinista ---------------------------------------------------
//
// El ruido tiene que ser una función pura de (semilla, instante). Nada de
// generadores con estado: si el osciloscopio evalúa el nanosegundo 500 antes
// que el 200, ambos deben dar el mismo valor que si se hubieran pedido en
// orden. Se consigue mezclando la semilla con el instante mediante una función
// de dispersión y transformando el resultado en una gaussiana.

/// Mezclador de 64 bits (splitmix64). Baratísimo y de buena calidad estadística.
fn mezclar(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Convierte 64 bits en un número real en el intervalo abierto (0, 1).
///
/// Se descartan los 11 bits bajos porque un `f64` solo tiene 53 bits de
/// mantisa. El desplazamiento de medio paso evita devolver exactamente cero,
/// que rompería el logaritmo de Box-Muller.
fn uniforme(bits: u64) -> f64 {
    ((bits >> 11) as f64 + 0.5) * (1.0 / 9_007_199_254_740_992.0)
}

/// Muestra de una gaussiana de media cero y desviación uno, determinista.
fn gaussiano(seed: u64, nanos: u64) -> f64 {
    let h1 = mezclar(seed ^ mezclar(nanos));
    let h2 = mezclar(h1);

    // Transformación de Box-Muller: dos uniformes independientes dan dos
    // gaussianas independientes. Solo necesitamos una.
    let u1 = uniforme(h1);
    let u2 = uniforme(h2);
    (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Margen para comparar reales. Comparar `f64` con `==` casi nunca es
    /// correcto: hay que comprobar que la diferencia es despreciable.
    fn casi_igual(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn la_constante_vale_lo_mismo_siempre() {
        let s = Signal::Constant(5.0);
        assert_eq!(s.eval(SimTime::ZERO), 5.0);
        assert_eq!(s.eval(SimTime::from_secs_f64(1e6)), 5.0);
    }

    #[test]
    fn la_senoide_pasa_por_sus_puntos_notables() {
        // 1 Hz, amplitud 1: en t=0 vale 0, en t=0,25 s vale 1, en 0,5 s vale 0.
        let s = Signal::sine(1.0, 1.0);
        assert!(casi_igual(s.eval(SimTime::ZERO), 0.0, 1e-12));
        assert!(casi_igual(s.eval(SimTime::from_secs_f64(0.25)), 1.0, 1e-9));
        assert!(casi_igual(s.eval(SimTime::from_secs_f64(0.75)), -1.0, 1e-9));
    }

    #[test]
    fn la_rampa_respeta_sus_extremos_y_el_punto_medio() {
        let s = Signal::Ramp {
            from: 0.0,
            to: 10.0,
            start: SimTime::from_secs_f64(1.0),
            duration: Duration::from_secs(2),
        };

        assert_eq!(s.eval(SimTime::ZERO), 0.0);                        // antes
        assert_eq!(s.eval(SimTime::from_secs_f64(1.0)), 0.0);          // justo al inicio
        assert!(casi_igual(s.eval(SimTime::from_secs_f64(2.0)), 5.0, 1e-9)); // mitad
        assert_eq!(s.eval(SimTime::from_secs_f64(3.0)), 10.0);         // final
        assert_eq!(s.eval(SimTime::from_secs_f64(99.0)), 10.0);        // después
    }

    #[test]
    fn la_cuadrada_conmuta_segun_el_ciclo_de_trabajo() {
        // 1 Hz, 25% de ciclo de trabajo, entre -1 y +1.
        let s = Signal::Square { amplitude: 1.0, frequency: 1.0, duty: 0.25, offset: 0.0 };
        assert_eq!(s.eval(SimTime::from_secs_f64(0.1)), 1.0);
        assert_eq!(s.eval(SimTime::from_secs_f64(0.4)), -1.0);
        assert_eq!(s.eval(SimTime::from_secs_f64(1.1)), 1.0); // periodo siguiente
    }

    #[test]
    fn el_pulso_solo_vale_dentro_de_su_ventana() {
        let s = Signal::Pulse {
            amplitude: 3.3,
            start: SimTime::from_secs_f64(1.0),
            width: Duration::from_millis(10),
            baseline: 0.0,
        };
        assert_eq!(s.eval(SimTime::from_secs_f64(0.999)), 0.0);
        assert_eq!(s.eval(SimTime::from_secs_f64(1.005)), 3.3);
        assert_eq!(s.eval(SimTime::from_secs_f64(1.011)), 0.0);
    }

    #[test]
    fn la_suma_suma() {
        let s = Signal::Sum(vec![Signal::Constant(2.0), Signal::Constant(3.0)]);
        assert_eq!(s.eval(SimTime::ZERO), 5.0);
    }

    #[test]
    fn la_ganancia_escala() {
        let s = Signal::Constant(2.0).scaled(1.5);
        assert_eq!(s.eval(SimTime::ZERO), 3.0);
    }

    /// La propiedad más importante del ruido: es una función del tiempo, no un
    /// flujo. Evaluar el mismo instante dos veces da el mismo valor, y el orden
    /// en que se evalúen los instantes es irrelevante.
    #[test]
    fn el_ruido_es_determinista_y_no_depende_del_orden() {
        let s = Signal::Noise { rms: 1.0, seed: 42 };

        let t1 = SimTime::from_nanos(200);
        let t2 = SimTime::from_nanos(500);

        let a = s.eval(t1);
        let b = s.eval(t2);

        // Al revés y repitiendo: mismos valores.
        assert_eq!(s.eval(t2), b);
        assert_eq!(s.eval(t1), a);
        assert_ne!(a, b, "dos instantes distintos deberían dar ruido distinto");
    }

    #[test]
    fn semillas_distintas_dan_ruidos_distintos() {
        let t = SimTime::from_nanos(12_345);
        let a = Signal::Noise { rms: 1.0, seed: 1 }.eval(t);
        let b = Signal::Noise { rms: 1.0, seed: 2 }.eval(t);
        assert_ne!(a, b);
    }

    #[test]
    fn el_ruido_tiene_la_media_y_el_valor_eficaz_pedidos() {
        let rms = 50e-6; // 50 µV, ruido típico de un buen multímetro
        let s = Signal::Noise { rms, seed: 7 };

        let n = 20_000;
        let mut muestras = vec![0.0; n];
        s.eval_block(SimTime::ZERO, Duration::from_micros(1), &mut muestras);

        let media = muestras.iter().sum::<f64>() / n as f64;
        let varianza = muestras.iter().map(|v| (v - media).powi(2)).sum::<f64>() / n as f64;

        assert!(media.abs() < rms * 0.05, "media desviada: {media}");
        assert!(casi_igual(varianza.sqrt(), rms, rms * 0.05), "rms: {}", varianza.sqrt());
    }

    #[test]
    fn la_forma_de_onda_interpola() {
        let w = Waveform::new(
            SimTime::ZERO,
            Duration::from_secs(1),
            vec![0.0, 10.0, 20.0],
        );

        assert_eq!(w.eval(SimTime::ZERO), 0.0);
        assert!(casi_igual(w.eval(SimTime::from_secs_f64(0.5)), 5.0, 1e-9));
        assert_eq!(w.eval(SimTime::from_secs_f64(2.0)), 20.0);
        assert_eq!(w.eval(SimTime::from_secs_f64(99.0)), 20.0); // se mantiene el extremo
    }

    /// El hito de la fase 1, y la justificación de todo este módulo.
    ///
    /// El motor corre a 1 kHz. Aun así el osciloscopio captura diez mil
    /// muestras de una senoide de 1 MHz separadas un nanosegundo, es decir a
    /// 1 GS/s, y salen exactamente diez periodos completos y correctos.
    #[test]
    fn captura_a_1_gigamuestra_por_segundo_con_el_motor_a_1_kilohercio() {
        let generador = Signal::sine(2.0, 1e6); // 2 V pico, 1 MHz

        // El motor lleva una hora corriendo cuando se dispara la captura.
        let disparo = SimTime::from_secs_f64(3600.0);

        let mut captura = vec![0.0; 10_000];
        generador.eval_block(disparo, Duration::from_nanos(1), &mut captura);

        // 10.000 muestras a 1 ns son 10 µs, y a 1 MHz eso son 10 periodos.
        // La muestra 0 y la 1000 caen en el mismo punto de la onda.
        assert!(casi_igual(captura[0], captura[1000], 1e-6));

        // Un cuarto de periodo son 250 ns: ahí está el máximo.
        assert!(casi_igual(captura[250], 2.0, 1e-6), "pico: {}", captura[250]);
        assert!(casi_igual(captura[750], -2.0, 1e-6), "valle: {}", captura[750]);

        // Y el recorrido completo es el esperado, sin pérdida de precisión pese
        // a estar en el segundo 3600 de simulación.
        let max = captura.iter().cloned().fold(f64::MIN, f64::max);
        let min = captura.iter().cloned().fold(f64::MAX, f64::min);
        assert!(casi_igual(max, 2.0, 1e-6));
        assert!(casi_igual(min, -2.0, 1e-6));
    }
}
