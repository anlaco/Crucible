//! Núcleo de simulación de InstruSim: tiempo, señales, mundo y triggers.
//!
//! `//!` es un comentario de documentación "hacia dentro": documenta el elemento
//! que lo contiene, en este caso el crate entero. Con `///` documentas el
//! elemento que viene justo después. `cargo doc --open` los convierte en web.

// Declara que existe un módulo `time` cuyo código está en `src/time.rs`.
// Sin esta línea, ese fichero simplemente no se compila: en Rust los módulos
// se declaran explícitamente, no se descubren por estar en la carpeta.
pub mod time;

// Reexporta `SimTime` en la raíz del crate, para que quien lo use pueda escribir
// `instrusim_core::SimTime` en vez de `instrusim_core::time::SimTime`.
pub use time::SimTime;
