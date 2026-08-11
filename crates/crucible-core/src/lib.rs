//! Crucible: el formato declarativo y su runtime de referencia.
//!
//! Un perfil describe **qué hace** un dispositivo; este crate lo carga, lo
//! valida y lo convierte en algo que responde por el protocolo que declare.
//!
//! **El protocolo no se implementa aquí.** SCPI vive en `instrusim-scpi`, que
//! es el único del repositorio y lo comparten los dispositivos declarativos y
//! los instrumentos escritos en Rust de `instrusim-model` (ADR-0003).

pub mod banco;
pub mod dispositivo;
pub mod error;
pub mod estado;
pub mod modelo;
pub mod perfil;
pub mod protocolo;

pub use banco::{Banco, InstanciaDispositivo, Transporte};
pub use dispositivo::Dispositivo;
pub use error::{CrucibleError, Result};
pub use estado::{Estado, Valor};
pub use modelo::EvaluadorModelos;
pub use perfil::{DispositivoInfo, Perfil};
pub use protocolo::scpi::{self, DispositivoScpi};
