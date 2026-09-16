//! Capa fina sobre el sistema operativo. Todo lo que toca Windows de verdad
//! vive aqui detras de una interfaz que el stub replica para el resto de
//! plataformas.

use gm_core::error::Result;

use crate::guid::SchemeGuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    /// Nombre del ejecutable sin extension.
    pub name: String,
    /// PID del proceso que lo lanzo. 0 si no se pudo determinar.
    pub parent_pid: u32,
}

/// Consumo de un proceso en un instante dado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessUsage {
    pub pid: u32,
    pub name: String,
    /// PID del proceso que lo lanzo. 0 si no se pudo determinar. Sirve para
    /// reconocer procesos hijos del juego (un lanzador que arranca el
    /// ejecutable real como hijo suyo, muy comun) y no degradarlos por error.
    pub parent_pid: u32,
    /// RAM fisica que ocupa ahora mismo.
    pub working_set: u64,
    /// Memoria comprometida (incluye lo paginado a disco).
    pub private_bytes: u64,
}

/// Estado de un servicio, con el proceso que lo aloja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatus {
    pub name: String,
    pub display: String,
    pub running: bool,
    /// 0 si no esta en ejecucion. Varios servicios comparten `svchost`, asi que
    /// la memoria asociada a un PID puede ser de varios a la vez.
    pub pid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceRunState {
    Running,
    Stopped,
    /// Arrancando, parandose, pausado...
    Transitioning,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStatus {
    pub total: u64,
    pub available: u64,
}

impl MemoryStatus {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
}

#[cfg(windows)]
mod imp;
#[cfg(not(windows))]
mod imp_stub;

#[cfg(windows)]
pub use imp::*;
#[cfg(not(windows))]
pub use imp_stub::*;

/// Comprueba que un plan de energia se puede activar; si es el plan "maximo
/// rendimiento" y no esta presente, intenta crearlo con powercfg.
pub fn ensure_scheme_available(scheme: SchemeGuid) -> Result<()> {
    if scheme != crate::guid::ULTIMATE {
        return Ok(());
    }
    duplicate_ultimate_scheme()
}
