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
}

/// Consumo de un proceso en un instante dado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessUsage {
    pub pid: u32,
    pub name: String,
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

/// Clases de prioridad de Windows (valores del SDK, replicados para poder
/// serializarlos en el snapshot sin depender del crate `windows`).
pub mod priority {
    pub const IDLE: u32 = 0x0000_0040;
    pub const BELOW_NORMAL: u32 = 0x0000_4000;
    pub const NORMAL: u32 = 0x0000_0020;
    pub const ABOVE_NORMAL: u32 = 0x0000_8000;
    pub const HIGH: u32 = 0x0000_0080;

    pub fn name(class: u32) -> &'static str {
        match class {
            IDLE => "baja",
            BELOW_NORMAL => "inferior a normal",
            NORMAL => "normal",
            ABOVE_NORMAL => "superior a normal",
            HIGH => "alta",
            _ => "desconocida",
        }
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
