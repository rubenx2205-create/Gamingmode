//! Nucleo compartido del modo juego: configuracion, biblioteca y lanzamiento.
//!
//! Este crate no depende de la interfaz ni del backend de entrada, de modo que
//! se puede compilar y testear en cualquier plataforma; lo especifico de
//! Windows va detras de `#[cfg(windows)]`.

pub mod config;
pub mod error;
pub mod launcher;
pub mod library;
pub mod paths;
pub mod util;

pub use config::Config;
pub use error::{Error, Result};
pub use library::{Game, Launch, Library, Sort};
