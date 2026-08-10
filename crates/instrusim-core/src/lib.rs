//! Núcleo de simulación de InstruSim: tiempo, señales, mundo y triggers.
//!
//! `//!` es un comentario de documentación "hacia dentro": documenta el elemento
//! que lo contiene, en este caso el crate entero. Con `///` documentas el
//! elemento que viene justo después. `cargo doc --open` los convierte en web.

pub mod clock;
pub mod signal;
pub mod time;

// Reexporta los tipos principales en la raíz del crate, para que quien los use
// pueda escribir `instrusim_core::SimTime` en vez de la ruta completa.
pub use clock::{Clock, VirtualClock, WallClock};
pub use signal::{Signal, Waveform};
pub use time::SimTime;
