//! Implementacion vacia para plataformas que no son Windows: permite compilar
//! y testear la maquina de estados del optimizador en cualquier sitio.

use gm_core::error::{Error, Result};

use super::{MemoryStatus, ProcInfo, ProcessUsage, ServiceRunState, ServiceStatus};
use crate::guid::SchemeGuid;

const MSG: &str = "el modo juego solo modifica el sistema en Windows";

pub fn is_elevated() -> bool {
    false
}

pub fn active_power_scheme() -> Result<SchemeGuid> {
    Err(Error::Unsupported(MSG))
}

pub fn set_power_scheme(_scheme: SchemeGuid) -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn duplicate_ultimate_scheme() -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn service_state(_name: &str) -> Result<ServiceRunState> {
    Err(Error::Unsupported(MSG))
}

pub fn stop_service(_name: &str) -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn start_service(_name: &str) -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn list_processes() -> Result<Vec<ProcInfo>> {
    Ok(Vec::new())
}

pub fn sample_processes() -> Result<Vec<ProcessUsage>> {
    Ok(Vec::new())
}

pub fn list_service_status() -> Result<Vec<ServiceStatus>> {
    Ok(Vec::new())
}

pub fn priority_class(_pid: u32) -> Result<u32> {
    Err(Error::Unsupported(MSG))
}

pub fn set_priority_class(_pid: u32, _class: u32) -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn set_eco_qos(_pid: u32, _enabled: bool) -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn trim_working_set(_pid: u32) -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn memory_status() -> Result<MemoryStatus> {
    Ok(MemoryStatus::default())
}

pub fn kill_explorer() -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn start_explorer() -> Result<()> {
    Err(Error::Unsupported(MSG))
}

pub fn current_pid() -> u32 {
    std::process::id()
}
