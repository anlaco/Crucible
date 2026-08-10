//! Capa SCPI-99 / IEEE 488.2 de InstruSim.
//!
//! Trabaja solo con texto: recibe la línea que llegó por la red y produce la
//! línea que hay que devolver. No sabe nada de instrumentos, de física ni de
//! sockets, y no depende de ningún crate externo.
//!
//! Está escrita a medida en vez de usar el crate `scpi` que existe en el
//! ecosistema porque aquél construye el árbol de comandos en tiempo de
//! compilación, pensado para embebido, y el diseño de InstruSim exige lo
//! contrario: cargar los comandos de un fichero TOML en tiempo de ejecución.

pub mod error;
pub mod format;
pub mod parse;
pub mod pattern;
pub mod status;
pub mod table;

pub use error::{ErrorCode, ErrorQueue, ScpiError};
pub use parse::{Command, Numeric, parse_message};
pub use pattern::Pattern;
pub use status::StatusModel;
pub use table::CommandTable;
