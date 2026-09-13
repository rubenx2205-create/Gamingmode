//! Motor de optimizacion de recursos del modo juego.
//!
//! Filosofia: **todo lo que se toca se apunta y se devuelve como estaba**. El
//! modo juego no es un "optimizador" de los que dejan el sistema hecho un
//! desastre; es un interruptor con vuelta atras garantizada, incluso si el
//! proceso muere sin avisar.

pub mod guid;
pub mod snapshot;
pub mod sys;

use std::path::PathBuf;

use gm_core::config::{Power, PowerPlan};
pub use snapshot::{ProcessState, ServiceState, Snapshot};
pub use sys::{memory_status, MemoryStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub outcome: Outcome,
    pub area: &'static str,
    pub message: String,
}

/// Lo que ha pasado al activar o revertir el modo juego. Se muestra tal cual en
/// la interfaz: el usuario tiene derecho a saber que se ha tocado en su equipo.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub entries: Vec<Entry>,
    pub elevated: bool,
    pub memory_before: u64,
    pub memory_after: u64,
}

impl Report {
    fn push(&mut self, outcome: Outcome, area: &'static str, message: impl Into<String>) {
        self.entries.push(Entry { outcome, area, message: message.into() });
    }

    fn applied(&mut self, area: &'static str, message: impl Into<String>) {
        self.push(Outcome::Applied, area, message);
    }

    fn skipped(&mut self, area: &'static str, message: impl Into<String>) {
        self.push(Outcome::Skipped, area, message);
    }

    fn failed(&mut self, area: &'static str, message: impl Into<String>) {
        self.push(Outcome::Failed, area, message);
    }

    pub fn count(&self, outcome: Outcome) -> usize {
        self.entries.iter().filter(|e| e.outcome == outcome).count()
    }

    /// RAM liberada (o consumida, si sale negativo) durante la operacion.
    pub fn memory_delta(&self) -> i64 {
        self.memory_after as i64 - self.memory_before as i64
    }

    pub fn summary(&self) -> String {
        let applied = self.count(Outcome::Applied);
        let failed = self.count(Outcome::Failed);
        let delta = self.memory_delta();
        let mut text = format!("{applied} cambios aplicados");
        if failed > 0 {
            text.push_str(&format!(", {failed} fallidos"));
        }
        if delta > 8 * 1024 * 1024 {
            text.push_str(&format!(", {} de RAM liberada", gm_core::util::format_bytes(delta as u64)));
        }
        text
    }
}

pub struct Optimizer {
    config: Power,
    snapshot_path: PathBuf,
    /// `Some` mientras el modo juego esta activo.
    engaged: Option<Snapshot>,
}

impl Optimizer {
    pub fn new(config: Power) -> Self {
        Self::with_snapshot_path(config, gm_core::paths::power_snapshot_file())
    }

    pub fn with_snapshot_path(config: Power, snapshot_path: PathBuf) -> Self {
        Self { config, snapshot_path, engaged: None }
    }

    pub fn set_config(&mut self, config: Power) {
        self.config = config;
    }

    pub fn config(&self) -> &Power {
        &self.config
    }

    pub fn is_engaged(&self) -> bool {
        self.engaged.is_some()
    }

    /// Al arrancar: si quedo un snapshot de una sesion anterior que no cerro
    /// bien, se revierte antes de hacer nada mas.
    pub fn recover_stale(&mut self) -> Option<Report> {
        let snapshot = match Snapshot::load(&self.snapshot_path) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => return None,
            Err(e) => {
                log::warn!("no se pudo leer el snapshot previo: {e}");
                return None;
            }
        };
        if snapshot.is_empty() {
            let _ = Snapshot::clear(&self.snapshot_path);
            return None;
        }
        log::warn!("se encontro un snapshot de una sesion anterior; revirtiendo");
        self.engaged = Some(snapshot);
        let mut report = self.restore();
        report.push(Outcome::Applied, "recuperacion", "se revirtio una sesion anterior que no cerro bien");
        Some(report)
    }

    /// Entra en modo juego.
    pub fn engage(&mut self) -> Report {
        let mut report = Report {
            elevated: sys::is_elevated(),
            memory_before: sys::memory_status().map(|m| m.used()).unwrap_or(0),
            ..Default::default()
        };

        if self.engaged.is_some() {
            report.skipped("estado", "el modo juego ya estaba activo");
            return report;
        }

        let mut snapshot = Snapshot::new();
        self.apply_power_plan(&mut snapshot, &mut report);
        self.capture_services(&mut snapshot, &mut report);

        // El snapshot se guarda ANTES de parar nada: si el equipo se va aqui,
        // la siguiente sesion sabra que restaurar.
        self.persist(&snapshot, &mut report);

        self.stop_services(&mut snapshot, &mut report);
        self.throttle_processes(&mut snapshot, &mut report);
        self.handle_explorer(&mut snapshot, &mut report);

        self.persist(&snapshot, &mut report);
        self.engaged = Some(snapshot);

        report.memory_after = sys::memory_status().map(|m| m.used()).unwrap_or(0);
        report
    }

    /// Devuelve el sistema a como estaba.
    pub fn restore(&mut self) -> Report {
        let mut report = Report {
            elevated: sys::is_elevated(),
            memory_before: sys::memory_status().map(|m| m.used()).unwrap_or(0),
            ..Default::default()
        };

        let Some(snapshot) = self.engaged.take() else {
            report.skipped("estado", "el modo juego no estaba activo");
            return report;
        };

        // Orden inverso al de activacion.
        if snapshot.explorer_killed {
            match sys::start_explorer() {
                Ok(()) => report.applied("escritorio", "explorer.exe relanzado"),
                Err(e) => report.failed("escritorio", format!("no se pudo relanzar explorer.exe: {e}")),
            }
        }

        let live: Vec<sys::ProcInfo> = sys::list_processes().unwrap_or_default();
        for process in &snapshot.processes {
            // Un PID se reutiliza; sin comprobar el nombre podriamos estar
            // cambiandole la prioridad a un proceso que no es el nuestro.
            let still_there = live.iter().any(|p| p.pid == process.pid && p.name == process.name);
            if !still_there {
                report.skipped("procesos", format!("{} ya no esta en ejecucion", process.name));
                continue;
            }
            if process.eco_qos_applied {
                let _ = sys::set_eco_qos(process.pid, false);
            }
            match sys::set_priority_class(process.pid, process.previous_priority) {
                Ok(()) => report.applied(
                    "procesos",
                    format!("{} vuelve a prioridad {}", process.name, sys::priority::name(process.previous_priority)),
                ),
                Err(e) => report.failed("procesos", format!("{}: {e}", process.name)),
            }
        }

        for service in &snapshot.services {
            if !service.was_running {
                report.skipped("servicios", format!("{} ya estaba parado antes", service.name));
                continue;
            }
            match sys::start_service(&service.name) {
                Ok(()) => report.applied("servicios", format!("{} rearrancado", service.name)),
                Err(e) => report.failed("servicios", format!("no se pudo rearrancar {}: {e}", service.name)),
            }
        }

        if let Some(scheme) = snapshot.power_scheme {
            match sys::set_power_scheme(scheme) {
                Ok(()) => report.applied("energia", format!("plan de energia restaurado ({scheme})")),
                Err(e) => report.failed("energia", format!("no se pudo restaurar el plan de energia: {e}")),
            }
        }

        if let Err(e) = Snapshot::clear(&self.snapshot_path) {
            report.failed("snapshot", format!("no se pudo borrar el snapshot: {e}"));
        }

        report.memory_after = sys::memory_status().map(|m| m.used()).unwrap_or(0);
        report
    }

    fn persist(&self, snapshot: &Snapshot, report: &mut Report) {
        if let Err(e) = snapshot.save(&self.snapshot_path) {
            report.failed("snapshot", format!("no se pudo guardar el estado previo: {e}"));
        }
    }

    fn apply_power_plan(&self, snapshot: &mut Snapshot, report: &mut Report) {
        let target = match self.config.power_plan {
            PowerPlan::Leave => {
                report.skipped("energia", "se respeta el plan de energia del usuario");
                return;
            }
            PowerPlan::Balanced => guid::BALANCED,
            PowerPlan::HighPerformance => guid::HIGH_PERFORMANCE,
            PowerPlan::Ultimate => guid::ULTIMATE,
        };

        let current = match sys::active_power_scheme() {
            Ok(scheme) => scheme,
            Err(e) => {
                report.failed("energia", format!("no se pudo leer el plan actual: {e}"));
                return;
            }
        };
        if current == target {
            report.skipped("energia", "el plan de energia pedido ya estaba activo");
            return;
        }
        snapshot.power_scheme = Some(current);

        // "Maximo rendimiento" puede no existir todavia en este equipo.
        if target == guid::ULTIMATE {
            if let Err(e) = sys::ensure_scheme_available(target) {
                log::debug!("no se pudo duplicar el plan maximo rendimiento: {e}");
            }
        }

        match sys::set_power_scheme(target) {
            Ok(()) => report.applied("energia", format!("plan de energia -> {:?}", self.config.power_plan)),
            Err(_) if target == guid::ULTIMATE => {
                // Los portatiles suelen no tener "maximo rendimiento".
                match sys::set_power_scheme(guid::HIGH_PERFORMANCE) {
                    Ok(()) => report.applied("energia", "plan de energia -> alto rendimiento (maximo no disponible)"),
                    Err(e) => {
                        snapshot.power_scheme = None;
                        report.failed("energia", format!("no se pudo cambiar el plan: {e}"));
                    }
                }
            }
            Err(e) => {
                snapshot.power_scheme = None;
                report.failed("energia", format!("no se pudo cambiar el plan: {e}"));
            }
        }
    }

    fn capture_services(&self, snapshot: &mut Snapshot, report: &mut Report) {
        if self.config.stop_services.is_empty() {
            return;
        }
        if !sys::is_elevated() {
            report.skipped("servicios", "hace falta ejecutar el modo juego como administrador para parar servicios");
            return;
        }
        for name in &self.config.stop_services {
            match sys::service_state(name) {
                Ok(state) => snapshot
                    .services
                    .push(ServiceState { name: name.clone(), was_running: state == sys::ServiceRunState::Running }),
                Err(e) => report.skipped("servicios", format!("{name}: {e}")),
            }
        }
    }

    fn stop_services(&self, snapshot: &mut Snapshot, report: &mut Report) {
        for service in &snapshot.services {
            if !service.was_running {
                report.skipped("servicios", format!("{} ya estaba parado", service.name));
                continue;
            }
            match sys::stop_service(&service.name) {
                Ok(()) => report.applied("servicios", format!("{} detenido", service.name)),
                Err(e) => report.failed("servicios", format!("no se pudo detener {}: {e}", service.name)),
            }
        }
    }

    fn throttle_processes(&self, snapshot: &mut Snapshot, report: &mut Report) {
        if self.config.throttle_processes.is_empty() {
            return;
        }
        let processes = match sys::list_processes() {
            Ok(processes) => processes,
            Err(e) => {
                report.failed("procesos", format!("no se pudo enumerar procesos: {e}"));
                return;
            }
        };
        let me = sys::current_pid();

        for process in processes {
            if process.pid == me || process.pid == 0 {
                continue;
            }
            if !self.config.throttle_processes.iter().any(|name| name.eq_ignore_ascii_case(&process.name)) {
                continue;
            }

            let previous = sys::priority_class(process.pid).unwrap_or(sys::priority::NORMAL);
            if previous == sys::priority::IDLE {
                continue; // ya estaba en minimos, no hay nada que guardar
            }
            if let Err(e) = sys::set_priority_class(process.pid, sys::priority::IDLE) {
                report.skipped("procesos", format!("{} (pid {}): {e}", process.name, process.pid));
                continue;
            }

            let eco = self.config.eco_qos_background && sys::set_eco_qos(process.pid, true).is_ok();
            if self.config.trim_working_sets {
                let _ = sys::trim_working_set(process.pid);
            }

            snapshot.processes.push(ProcessState {
                pid: process.pid,
                name: process.name.clone(),
                previous_priority: previous,
                eco_qos_applied: eco,
            });
            report.applied(
                "procesos",
                format!("{} en segundo plano{}", process.name, if eco { " (EcoQoS)" } else { "" }),
            );
        }
    }

    fn handle_explorer(&self, snapshot: &mut Snapshot, report: &mut Report) {
        if !self.config.kill_explorer {
            return;
        }
        match sys::kill_explorer() {
            Ok(()) => {
                snapshot.explorer_killed = true;
                report.applied("escritorio", "explorer.exe cerrado (se relanzara al salir)");
            }
            Err(e) => report.failed("escritorio", format!("no se pudo cerrar explorer.exe: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gm-power-{}-{tag}.json", std::process::id()))
    }

    #[test]
    fn activar_dos_veces_no_duplica_el_estado() {
        let path = temp_path("doble");
        let _ = std::fs::remove_file(&path);
        let mut optimizer = Optimizer::with_snapshot_path(Power::default(), path.clone());

        optimizer.engage();
        assert!(optimizer.is_engaged());
        let second = optimizer.engage();
        assert!(second.entries.iter().any(|e| e.outcome == Outcome::Skipped && e.area == "estado"));

        optimizer.restore();
        assert!(!optimizer.is_engaged());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn restaurar_borra_el_snapshot_de_disco() {
        let path = temp_path("limpieza");
        let _ = std::fs::remove_file(&path);
        let mut optimizer = Optimizer::with_snapshot_path(Power::default(), path.clone());
        optimizer.engage();
        optimizer.restore();
        assert!(!path.exists(), "el snapshot deberia desaparecer tras restaurar");
    }

    #[test]
    fn se_revierte_un_snapshot_huerfano_al_arrancar() {
        let path = temp_path("huerfano");
        let mut stale = Snapshot::new();
        stale.services.push(ServiceState { name: "SysMain".into(), was_running: true });
        stale.save(&path).unwrap();

        let mut optimizer = Optimizer::with_snapshot_path(Power::default(), path.clone());
        let report = optimizer.recover_stale().expect("deberia detectar el snapshot huerfano");
        assert!(report.entries.iter().any(|e| e.area == "recuperacion"));
        assert!(!optimizer.is_engaged());
        assert!(!path.exists());
    }

    #[test]
    fn sin_snapshot_previo_no_hay_recuperacion() {
        let path = temp_path("vacio");
        let _ = std::fs::remove_file(&path);
        let mut optimizer = Optimizer::with_snapshot_path(Power::default(), path);
        assert!(optimizer.recover_stale().is_none());
    }

    #[test]
    fn restaurar_sin_activar_avisa_en_vez_de_fallar() {
        let mut optimizer = Optimizer::with_snapshot_path(Power::default(), temp_path("sin-activar"));
        let report = optimizer.restore();
        assert!(report.entries.iter().any(|e| e.outcome == Outcome::Skipped));
    }
}
