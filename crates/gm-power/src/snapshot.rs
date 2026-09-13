//! Foto del estado del sistema antes de entrar en modo juego.
//!
//! Se escribe a disco *antes* de tocar nada. Si el shell se cierra de golpe (o
//! el equipo se apaga), al siguiente arranque se encuentra el fichero y se
//! revierte todo: nadie quiere descubrir tres dias despues que Windows Search
//! lleva parado desde la ultima partida.

use std::path::Path;

use gm_core::error::Result;
use gm_core::util;
use serde::{Deserialize, Serialize};

use crate::guid::SchemeGuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    /// Estaba corriendo antes de que entrasemos?
    pub was_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessState {
    pub pid: u32,
    /// Se guarda el nombre para no restaurar la prioridad de un proceso que
    /// simplemente heredo el PID de otro que murio.
    pub name: String,
    pub previous_priority: u32,
    pub eco_qos_applied: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub taken_at: u64,
    /// Version del formato, por si el fichero sobrevive a una actualizacion.
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub power_scheme: Option<SchemeGuid>,
    #[serde(default)]
    pub services: Vec<ServiceState>,
    #[serde(default)]
    pub processes: Vec<ProcessState>,
    #[serde(default)]
    pub explorer_killed: bool,
}

fn default_version() -> u32 {
    1
}

impl Snapshot {
    pub fn new() -> Self {
        Self { taken_at: gm_core::library::now_secs(), version: default_version(), ..Default::default() }
    }

    pub fn load(path: &Path) -> Result<Option<Self>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        util::write_atomic(path, &serde_json::to_vec_pretty(self)?)
    }

    pub fn clear(path: &Path) -> Result<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Hay algo que revertir?
    pub fn is_empty(&self) -> bool {
        self.power_scheme.is_none() && self.services.is_empty() && self.processes.is_empty() && !self.explorer_killed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_a_disco() {
        let dir = std::env::temp_dir().join(format!("gm-snap-{}", std::process::id()));
        let path = dir.join("power-snapshot.json");
        let mut snapshot = Snapshot::new();
        snapshot.power_scheme = Some(crate::guid::BALANCED);
        snapshot.services.push(ServiceState { name: "SysMain".into(), was_running: true });
        snapshot.save(&path).unwrap();

        let loaded = Snapshot::load(&path).unwrap().unwrap();
        assert_eq!(loaded.power_scheme, Some(crate::guid::BALANCED));
        assert_eq!(loaded.services.len(), 1);
        assert!(!loaded.is_empty());

        Snapshot::clear(&path).unwrap();
        assert!(Snapshot::load(&path).unwrap().is_none());
        // Borrar dos veces no es un error.
        Snapshot::clear(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
