//! Núcleo de simulación de InstruSim.
//!
//! Contiene el motor y nada más: tiempo, reloj, señales, mundo y disparos. No
//! sabe nada de SCPI, de red ni de instrumentos concretos; esas capas se apoyan
//! encima. El núcleo no tiene dependencias externas.
//!
//! ```
//! use std::time::Duration;
//! use instrusim_core::{Engine, Signal, VirtualClock};
//!
//! let mut motor = Engine::new(Box::new(VirtualClock::from_hz(1000.0)));
//!
//! // Un nodo con una fuente de 5 V y 50 µV de ruido.
//! let salida = motor
//!     .world_mut()
//!     .add_node_with("fuente_out", Signal::Constant(5.0).with_noise(50e-6, 1));
//!
//! motor.run_for(Duration::from_millis(10));
//!
//! let medida = motor.world().potential_now(salida);
//! assert!((medida - 5.0).abs() < 1e-3);
//! ```

pub mod clock;
pub mod engine;
pub mod signal;
pub mod time;
pub mod trigger;
pub mod world;

// Reexporta los tipos principales en la raíz del crate, para que quien los use
// pueda escribir `instrusim_core::SimTime` en vez de la ruta completa.
pub use clock::{Clock, VirtualClock, WallClock};
pub use engine::{Engine, Stepper};
pub use signal::{Signal, Waveform};
pub use time::SimTime;
pub use trigger::{Edge, LineId, TriggerBus, TriggerEvent};
pub use world::{Node, NodeId, Terminal, World};
