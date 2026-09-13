//! Foto de lo que esta consumiendo recursos ahora mismo.
//!
//! Es lo que se ensena en la vista "Recursos" y lo que alimenta la seleccion
//! automatica: antes de tocar nada, el usuario puede ver exactamente que se va
//! a degradar, que se va a parar y que se va a respetar.

use gm_core::config::Power;

use crate::protect::Helper;
use crate::select::{self, Selection};
use crate::services::{self, ServiceEntry};
use crate::sys::{self, MemoryStatus, ProcessUsage, ServiceStatus};

/// Un servicio del catalogo seguro, con lo que esta gastando de verdad.
#[derive(Debug, Clone)]
pub struct ServiceReport {
    pub entry: &'static ServiceEntry,
    pub running: bool,
    /// RAM del proceso que lo aloja. Aproximada: varios servicios comparten
    /// `svchost` y entonces la cifra es la del grupo entero.
    pub ram: Option<u64>,
    pub shared_process: bool,
    /// Esta marcado para detenerse con la configuracion actual.
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SystemScan {
    /// Procesos ordenados de mas a menos RAM.
    pub processes: Vec<ProcessUsage>,
    pub services: Vec<ServiceReport>,
    /// Utilidades de portatil detectadas y respetadas.
    pub helpers: Vec<Helper>,
    /// Que se degradaria si se activase el modo juego ahora mismo.
    pub selection: Selection,
    pub memory: MemoryStatus,
    pub elevated: bool,
}

impl SystemScan {
    /// RAM que ocupan los servicios que se detendrian.
    pub fn recoverable_service_ram(&self) -> u64 {
        self.services
            .iter()
            .filter(|report| report.enabled && report.running && !report.shared_process)
            .filter_map(|report| report.ram)
            .sum()
    }
}

/// Cruza el catalogo de servicios con lo que hay corriendo.
///
/// Separado de las llamadas al sistema para poder probarlo en cualquier sitio.
pub fn build_service_reports(
    statuses: &[ServiceStatus],
    processes: &[ProcessUsage],
    enabled: &[String],
) -> Vec<ServiceReport> {
    let mut reports: Vec<ServiceReport> = services::CATALOG
        .iter()
        .map(|entry| {
            let status = statuses.iter().find(|status| status.name.eq_ignore_ascii_case(entry.name));
            let running = status.map(|status| status.running).unwrap_or(false);
            let pid = status.map(|status| status.pid).unwrap_or(0);
            let ram = if running && pid != 0 {
                processes.iter().find(|process| process.pid == pid).map(|process| process.working_set)
            } else {
                None
            };
            // Si otro servicio en ejecucion comparte PID, la memoria es del
            // grupo: pararlo no la devuelve entera.
            let shared_process =
                pid != 0 && statuses.iter().filter(|other| other.running && other.pid == pid).count() > 1;
            ServiceReport {
                entry,
                running,
                ram,
                shared_process,
                enabled: enabled.iter().any(|name| name.eq_ignore_ascii_case(entry.name)),
            }
        })
        .collect();

    // Primero lo que esta corriendo, y dentro de eso lo que mas pesa: asi la
    // lista se lee de arriba abajo por orden de interes.
    reports.sort_by(|a, b| {
        b.running
            .cmp(&a.running)
            .then(b.ram.unwrap_or(0).cmp(&a.ram.unwrap_or(0)))
            .then(a.entry.display.cmp(b.entry.display))
    });
    reports
}

/// Mide el equipo. Puede tardar unas decimas: conviene llamarla desde un hilo.
pub fn scan(config: &Power, enabled_services: &[String], game_pid: Option<u32>) -> SystemScan {
    let mut processes = sys::sample_processes().unwrap_or_default();
    processes.sort_by(|a, b| b.working_set.cmp(&a.working_set).then(a.name.cmp(&b.name)));

    let statuses = sys::list_service_status().unwrap_or_default();
    let helpers = crate::protect::detect_helpers(processes.iter().map(|process| process.name.as_str()));
    let selection = select::select_throttle_targets(&processes, config, sys::current_pid(), game_pid);

    SystemScan {
        services: build_service_reports(&statuses, &processes, enabled_services),
        helpers,
        selection,
        memory: sys::memory_status().unwrap_or_default(),
        elevated: sys::is_elevated(),
        processes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(name: &str, running: bool, pid: u32) -> ServiceStatus {
        ServiceStatus { name: name.to_string(), display: name.to_string(), running, pid }
    }

    fn process(pid: u32, name: &str, mb: u64) -> ProcessUsage {
        ProcessUsage { pid, name: name.to_string(), working_set: mb * 1024 * 1024, private_bytes: 0 }
    }

    #[test]
    fn cruza_servicios_con_la_memoria_de_su_proceso() {
        let statuses = vec![status("SysMain", true, 500), status("WSearch", true, 600)];
        let processes = vec![process(500, "svchost", 300), process(600, "SearchIndexer", 700)];
        let reports = build_service_reports(&statuses, &processes, &["SysMain".to_string()]);

        let sysmain = reports.iter().find(|r| r.entry.name == "SysMain").unwrap();
        assert!(sysmain.running && sysmain.enabled);
        assert_eq!(sysmain.ram, Some(300 * 1024 * 1024));
        assert!(!sysmain.shared_process);

        let wsearch = reports.iter().find(|r| r.entry.name == "WSearch").unwrap();
        assert!(wsearch.running);
        assert!(!wsearch.enabled, "no estaba en la seleccion");
    }

    #[test]
    fn marca_los_servicios_que_comparten_proceso() {
        // Dos servicios en el mismo svchost: la RAM no es de uno solo.
        let statuses = vec![status("SysMain", true, 500), status("DiagTrack", true, 500)];
        let processes = vec![process(500, "svchost", 400)];
        let reports = build_service_reports(&statuses, &processes, &[]);
        assert!(reports.iter().filter(|r| r.running).all(|r| r.shared_process));
    }

    #[test]
    fn un_servicio_que_no_existe_en_el_equipo_sale_parado_y_sin_memoria() {
        let reports = build_service_reports(&[], &[], &[]);
        assert_eq!(reports.len(), services::CATALOG.len());
        assert!(reports.iter().all(|r| !r.running && r.ram.is_none()));
    }

    #[test]
    fn la_lista_pone_primero_lo_que_esta_corriendo_y_pesa() {
        let statuses = vec![status("SysMain", true, 1), status("Spooler", true, 2), status("Fax", false, 0)];
        let processes = vec![process(1, "a", 100), process(2, "b", 300)];
        let reports = build_service_reports(&statuses, &processes, &[]);
        assert_eq!(reports[0].entry.name, "Spooler", "el que mas pesa primero");
        assert_eq!(reports[1].entry.name, "SysMain");
        assert!(!reports.last().unwrap().running);
    }

    #[test]
    fn solo_se_cuenta_como_recuperable_lo_que_no_comparte_proceso() {
        let scan = SystemScan {
            services: build_service_reports(
                &[status("SysMain", true, 1), status("Spooler", true, 2), status("DiagTrack", true, 2)],
                &[process(1, "a", 100), process(2, "b", 300)],
                &["SysMain".to_string(), "Spooler".to_string(), "DiagTrack".to_string()],
            ),
            ..Default::default()
        };
        // Spooler y DiagTrack comparten el PID 2: no se promete esa memoria.
        assert_eq!(scan.recoverable_service_ram(), 100 * 1024 * 1024);
    }
}
