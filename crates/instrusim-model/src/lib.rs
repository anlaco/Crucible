//! Modelos de instrumento de InstruSim.
//!
//! Cada instrumento implementa el trait [`Instrument`], que le obliga a exponer
//! identidad, registros de estado y cola de errores. Los comandos comunes de
//! IEEE 488.2 los resuelve [`handle_message`] una sola vez para todos, así que
//! un modelo nuevo solo escribe su árbol SCPI propio.
//!
//! La regla que ninguno rompe: **el instrumento no inventa lo que mide**. Lee
//! del mundo, y lo que lee depende de lo que otros instrumentos hayan puesto
//! ahí. Ese acoplamiento a través del mundo es lo que convierte una colección
//! de simuladores en un rack.

pub mod dmm;
pub mod instrument;
pub mod psu;
pub mod rack;

pub use dmm::{Accuracy, GenericDmm};
pub use instrument::{Identity, Instrument, handle_message};
pub use psu::GenericDcSupply;
pub use rack::{InstrumentId, Rack};
