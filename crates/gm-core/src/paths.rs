//! Rutas de datos del usuario.
//!
//! En Windows todo cuelga de `%APPDATA%\GamingMode`. En Linux (que es donde se
//! compila y se testea el nucleo) se usa la convencion XDG para poder ejecutar
//! las pruebas sin tocar el sistema real.

use std::path::PathBuf;

use crate::error::Result;

/// Permite redirigir todo el arbol de datos, util en tests y en modo portable.
pub const DATA_DIR_ENV: &str = "GAMINGMODE_DATA_DIR";

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        return PathBuf::from(dir);
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("GamingMode");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("gamingmode");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/gamingmode");
    }
    PathBuf::from("gamingmode-data")
}

pub fn config_file() -> PathBuf {
    data_dir().join("config.toml")
}

pub fn library_file() -> PathBuf {
    data_dir().join("library.json")
}

/// Estado previo del sistema antes de aplicar el modo juego. Se escribe en
/// disco *antes* de tocar nada para poder revertir tras un cierre inesperado.
pub fn power_snapshot_file() -> PathBuf {
    data_dir().join("power-snapshot.json")
}

pub fn covers_dir() -> PathBuf {
    data_dir().join("covers")
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn log_file() -> PathBuf {
    data_dir().join("gamingmode.log")
}

pub fn ensure_dirs() -> Result<()> {
    for dir in [data_dir(), covers_dir(), cache_dir()] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
