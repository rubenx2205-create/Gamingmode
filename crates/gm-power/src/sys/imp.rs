//! Implementacion real sobre la API de Win32.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use gm_core::error::{Error, Result};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE, HLOCAL};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Power::{PowerGetActiveScheme, PowerSetActiveScheme};
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Registry::HKEY;
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatus, StartServiceW, SC_HANDLE,
    SC_MANAGER_CONNECT, SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS,
    SERVICE_STOP, SERVICE_STOPPED,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetPriorityClass, OpenProcess, OpenProcessToken, ProcessPowerThrottling, SetPriorityClass,
    SetProcessInformation, TerminateProcess, PROCESS_ACCESS_RIGHTS, PROCESS_CREATION_FLAGS,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
};

use super::{MemoryStatus, ProcInfo, ServiceRunState};
use crate::guid::SchemeGuid;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn to_guid(scheme: SchemeGuid) -> GUID {
    GUID { data1: scheme.data1, data2: scheme.data2, data3: scheme.data3, data4: scheme.data4 }
}

fn from_guid(guid: GUID) -> SchemeGuid {
    SchemeGuid { data1: guid.data1, data2: guid.data2, data3: guid.data3, data4: guid.data4 }
}

/// Abre un proceso y garantiza el cierre del handle.
fn with_process<T>(pid: u32, access: PROCESS_ACCESS_RIGHTS, f: impl FnOnce(HANDLE) -> Result<T>) -> Result<T> {
    unsafe {
        let handle = OpenProcess(access, false, pid).map_err(|_| Error::Os { call: "OpenProcess", code: pid })?;
        let result = f(handle);
        let _ = CloseHandle(handle);
        result
    }
}

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

pub fn active_power_scheme() -> Result<SchemeGuid> {
    unsafe {
        let mut ptr: *mut GUID = std::ptr::null_mut();
        let status = PowerGetActiveScheme(HKEY::default(), &mut ptr);
        if status != ERROR_SUCCESS {
            return Err(Error::Os { call: "PowerGetActiveScheme", code: status.0 });
        }
        if ptr.is_null() {
            return Err(Error::Os { call: "PowerGetActiveScheme", code: 0 });
        }
        let guid = *ptr;
        // El GUID lo asigna el sistema con LocalAlloc.
        let _ = LocalFree(HLOCAL(ptr as *mut c_void));
        Ok(from_guid(guid))
    }
}

pub fn set_power_scheme(scheme: SchemeGuid) -> Result<()> {
    unsafe {
        let guid = to_guid(scheme);
        let status = PowerSetActiveScheme(HKEY::default(), Some(&guid));
        if status != ERROR_SUCCESS {
            return Err(Error::Os { call: "PowerSetActiveScheme", code: status.0 });
        }
        Ok(())
    }
}

/// El plan "maximo rendimiento" viene oculto; duplicarlo lo hace visible y
/// activable. Si ya existe, powercfg devuelve error y no pasa nada.
pub fn duplicate_ultimate_scheme() -> Result<()> {
    let output = std::process::Command::new("powercfg")
        .args(["-duplicatescheme", &crate::guid::ULTIMATE.to_string()])
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Os { call: "powercfg -duplicatescheme", code: output.status.code().unwrap_or(-1) as u32 })
    }
}

struct ServiceHandles {
    manager: SC_HANDLE,
    service: SC_HANDLE,
}

impl Drop for ServiceHandles {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.service);
            let _ = CloseServiceHandle(self.manager);
        }
    }
}

fn open_service(name: &str, access: u32) -> Result<ServiceHandles> {
    unsafe {
        let manager = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
            .map_err(|e| Error::Os { call: "OpenSCManagerW", code: e.code().0 as u32 })?;
        let name_w = wide(name);
        match OpenServiceW(manager, PCWSTR(name_w.as_ptr()), access) {
            Ok(service) => Ok(ServiceHandles { manager, service }),
            Err(e) => {
                let _ = CloseServiceHandle(manager);
                Err(Error::Os { call: "OpenServiceW", code: e.code().0 as u32 })
            }
        }
    }
}

fn query_status(service: SC_HANDLE) -> Result<SERVICE_STATUS> {
    unsafe {
        let mut status = SERVICE_STATUS::default();
        QueryServiceStatus(service, &mut status)
            .map_err(|e| Error::Os { call: "QueryServiceStatus", code: e.code().0 as u32 })?;
        Ok(status)
    }
}

pub fn service_state(name: &str) -> Result<ServiceRunState> {
    let handles = open_service(name, SERVICE_QUERY_STATUS)?;
    let status = query_status(handles.service)?;
    // Los "enum" del crate windows son structs envolviendo un entero, asi que
    // se comparan, no se hace match.
    let state = status.dwCurrentState;
    Ok(if state == SERVICE_RUNNING {
        ServiceRunState::Running
    } else if state == SERVICE_STOPPED {
        ServiceRunState::Stopped
    } else {
        ServiceRunState::Transitioning
    })
}

pub fn stop_service(name: &str) -> Result<()> {
    let handles = open_service(name, SERVICE_QUERY_STATUS | SERVICE_STOP)?;
    unsafe {
        let mut status = SERVICE_STATUS::default();
        ControlService(handles.service, SERVICE_CONTROL_STOP, &mut status)
            .map_err(|e| Error::Os { call: "ControlService", code: e.code().0 as u32 })?;
    }
    // Esperar a que pare de verdad, pero sin bloquear indefinidamente: algunos
    // servicios tardan y otros simplemente se niegan.
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if query_status(handles.service)?.dwCurrentState == SERVICE_STOPPED {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

pub fn start_service(name: &str) -> Result<()> {
    let handles = open_service(name, SERVICE_QUERY_STATUS | SERVICE_START)?;
    unsafe {
        StartServiceW(handles.service, None)
            .map_err(|e| Error::Os { call: "StartServiceW", code: e.code().0 as u32 })?;
    }
    Ok(())
}

pub fn list_processes() -> Result<Vec<ProcInfo>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| Error::Os { call: "CreateToolhelp32Snapshot", code: e.code().0 as u32 })?;
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut processes = Vec::with_capacity(256);
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                let name = name.strip_suffix(".exe").unwrap_or(&name).to_string();
                processes.push(ProcInfo { pid: entry.th32ProcessID, name });
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        Ok(processes)
    }
}

pub fn priority_class(pid: u32) -> Result<u32> {
    with_process(pid, PROCESS_QUERY_LIMITED_INFORMATION, |handle| {
        let class = unsafe { GetPriorityClass(handle) };
        if class == 0 {
            Err(Error::Os { call: "GetPriorityClass", code: pid })
        } else {
            Ok(class)
        }
    })
}

pub fn set_priority_class(pid: u32, class: u32) -> Result<()> {
    with_process(pid, PROCESS_SET_INFORMATION, |handle| unsafe {
        SetPriorityClass(handle, PROCESS_CREATION_FLAGS(class))
            .map_err(|e| Error::Os { call: "SetPriorityClass", code: e.code().0 as u32 })
    })
}

/// EcoQoS: el planificador manda el proceso a los nucleos eficientes y le baja
/// la frecuencia. Es la forma moderna (y reversible) de quitar de en medio a
/// los procesos de fondo sin matarlos.
pub fn set_eco_qos(pid: u32, enabled: bool) -> Result<()> {
    with_process(pid, PROCESS_SET_INFORMATION, |handle| unsafe {
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            // ControlMask a 0 devuelve el proceso al comportamiento por defecto
            // del sistema, que es justo lo que queremos al restaurar.
            ControlMask: if enabled { PROCESS_POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
            StateMask: if enabled { PROCESS_POWER_THROTTLING_EXECUTION_SPEED } else { 0 },
        };
        SetProcessInformation(
            handle,
            ProcessPowerThrottling,
            &state as *const _ as *const c_void,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
        .map_err(|e| Error::Os { call: "SetProcessInformation", code: e.code().0 as u32 })
    })
}

pub fn trim_working_set(pid: u32) -> Result<()> {
    with_process(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA, |handle| unsafe {
        EmptyWorkingSet(handle).map_err(|e| Error::Os { call: "EmptyWorkingSet", code: e.code().0 as u32 })
    })
}

pub fn memory_status() -> Result<MemoryStatus> {
    unsafe {
        let mut status =
            MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        GlobalMemoryStatusEx(&mut status)
            .map_err(|e| Error::Os { call: "GlobalMemoryStatusEx", code: e.code().0 as u32 })?;
        Ok(MemoryStatus { total: status.ullTotalPhys, available: status.ullAvailPhys })
    }
}

pub fn kill_explorer() -> Result<()> {
    let mut killed = false;
    for process in list_processes()?.into_iter().filter(|p| p.name.eq_ignore_ascii_case("explorer")) {
        let result = with_process(process.pid, PROCESS_TERMINATE, |handle| unsafe {
            TerminateProcess(handle, 0).map_err(|e| Error::Os { call: "TerminateProcess", code: e.code().0 as u32 })
        });
        killed |= result.is_ok();
    }
    if killed {
        Ok(())
    } else {
        Err(Error::NotFound("explorer.exe".to_string()))
    }
}

pub fn start_explorer() -> Result<()> {
    std::process::Command::new("explorer.exe").spawn()?;
    Ok(())
}

pub fn current_pid() -> u32 {
    std::process::id()
}
