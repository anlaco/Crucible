//! Capa SCPI-99 / IEEE 488.2 del proyecto.
//!
//! Trabaja solo con texto: recibe la línea que llegó por la red y produce la
//! línea que hay que devolver. No sabe nada de instrumentos, de física ni de
//! sockets, y no depende de ningún crate externo.
//!
//! Está escrita a medida en vez de usar el crate `scpi` que existe en el
//! ecosistema porque aquél construye el árbol de comandos en tiempo de
//! compilación, pensado para embebido, y el diseño exige lo contrario: cargar
//! los comandos de un fichero en tiempo de ejecución.
//!
//! **Es el único SCPI del repositorio.** Lo usan los dos linajes: los
//! instrumentos escritos en Rust de `instrusim-model` y los descritos en un
//! perfil YAML de `crucible-core`. El despacho de lo obligatorio vive en
//! [`device`], detrás del contrato [`ScpiDevice`], que no supone nada sobre
//! cómo esté implementado el dispositivo.

pub mod device;
pub mod error;
pub mod format;
pub mod parse;
pub mod pattern;
pub mod status;
pub mod table;

pub use device::{ScpiDevice, handle_message};
pub use error::{ErrorCode, ErrorQueue, ScpiError};
pub use parse::{Command, Numeric, parse_message};
pub use pattern::Pattern;
pub use status::StatusModel;
pub use table::CommandTable;
