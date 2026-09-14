//! Que degradar y que no.
//!
//! La version anterior llevaba una lista de nombres de programas conocidos.
//! Eso solo acierta en el equipo de quien escribio la lista: en el tuyo lo que
//! pesa es otra cosa. Aqui se mide lo que hay corriendo **ahora**, se aparta
//! todo lo protegido y se degrada lo mas pesado que quede.

use std::collections::HashMap;

use gm_core::config::Power;

use crate::protect::{protection_for, Protection};
use crate::sys::ProcessUsage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub pid: u32,
    pub name: String,
    pub working_set: u64,
    /// Puesto a mano por el usuario en `extra_throttle`: entra pese a no estar
    /// entre los mas pesados.
    pub forced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untouched {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub targets: Vec<Target>,
    /// Procesos respetados, sin repetir nombre (hay 40 `svchost` y no hacen
    /// falta 40 lineas en el informe).
    pub untouched: Vec<Untouched>,
}

impl Selection {
    fn note_untouched(&mut self, name: &str, protection: &Protection) {
        if self.untouched.iter().any(|other| other.name.eq_ignore_ascii_case(name)) {
            return;
        }
        self.untouched.push(Untouched { name: name.to_string(), reason: protection.reason() });
    }

    /// RAM total de los procesos elegidos.
    pub fn total_working_set(&self) -> u64 {
        self.targets.iter().map(|target| target.working_set).sum()
    }
}

/// Sube por la cadena de padres de `pid` buscando `game_pid`. Protege no solo
/// el proceso exacto que se lanzo, sino tambien a sus hijos: un lanzador
/// intermedio (Steam, un `.bat`, un instalador que arranca el juego real como
/// proceso hijo) es la norma, no la excepcion, y sin esto el juego de verdad
/// se quedaba fuera de la proteccion y podia acabar el mismo en el ranking de
/// "mas pesado".
///
/// Limitada a una profundidad corta: una cadena de padres corrupta o con un
/// PID reciclado (Windows los reutiliza) no debe poder colgar esto en un
/// bucle. 16 saltos es mas que de sobra para cualquier arbol de procesos real.
fn is_game_or_descendant(pid: u32, game_pid: u32, parents: &HashMap<u32, u32>) -> bool {
    let mut current = pid;
    for _ in 0..16 {
        if current == game_pid {
            return true;
        }
        match parents.get(&current) {
            Some(&parent) if parent != 0 && parent != current => current = parent,
            _ => return false,
        }
    }
    false
}

/// Elige los procesos a degradar.
///
/// `game_pid` se protege explicitamente, junto con toda su descendencia:
/// seria de chiste que el modo juego le bajase la prioridad al juego.
pub fn select_throttle_targets(
    processes: &[ProcessUsage],
    config: &Power,
    self_pid: u32,
    game_pid: Option<u32>,
) -> Selection {
    let mut selection = Selection::default();
    let mut ranked: Vec<&ProcessUsage> = Vec::new();
    let threshold = config.auto_throttle_min_mb * 1024 * 1024;
    let parents: HashMap<u32, u32> = processes.iter().map(|p| (p.pid, p.parent_pid)).collect();

    for process in processes {
        if process.pid == 0 {
            continue;
        }
        if process.pid == self_pid {
            selection.note_untouched(&process.name, &Protection::Own("es el propio modo juego".into()));
            continue;
        }
        if game_pid.is_some_and(|game_pid| is_game_or_descendant(process.pid, game_pid, &parents)) {
            selection.note_untouched(&process.name, &Protection::Own("es el juego en marcha".into()));
            continue;
        }
        if let Some(protection) =
            protection_for(&process.name, &config.protected_processes, config.protect_handheld_helpers)
        {
            selection.note_untouched(&process.name, &protection);
            continue;
        }

        if config.extra_throttle.iter().any(|entry| entry.eq_ignore_ascii_case(&process.name)) {
            selection.targets.push(Target {
                pid: process.pid,
                name: process.name.clone(),
                working_set: process.working_set,
                forced: true,
            });
            continue;
        }

        if config.auto_throttle && process.working_set >= threshold {
            ranked.push(process);
        }
    }

    // De mas pesado a menos; a igualdad, por nombre para que el informe salga
    // siempre igual y se pueda comparar entre sesiones.
    ranked.sort_by(|a, b| b.working_set.cmp(&a.working_set).then(a.name.cmp(&b.name)));
    ranked.truncate(config.auto_throttle_max);

    selection.targets.extend(ranked.into_iter().map(|process| Target {
        pid: process.pid,
        name: process.name.clone(),
        working_set: process.working_set,
        forced: false,
    }));

    selection
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, name: &str, mb: u64) -> ProcessUsage {
        child_process(pid, 0, name, mb)
    }

    fn child_process(pid: u32, parent_pid: u32, name: &str, mb: u64) -> ProcessUsage {
        ProcessUsage {
            pid,
            parent_pid,
            name: name.to_string(),
            working_set: mb * 1024 * 1024,
            private_bytes: mb * 1024 * 1024,
        }
    }

    fn config() -> Power {
        Power { auto_throttle_max: 2, auto_throttle_min_mb: 100, ..Power::default() }
    }

    #[test]
    fn se_eligen_los_mas_pesados_hasta_el_maximo() {
        let processes = vec![
            process(10, "navegador", 900),
            process(11, "chat", 400),
            process(12, "editor", 250),
            process(13, "cosita", 5),
        ];
        let selection = select_throttle_targets(&processes, &config(), 1, None);
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["navegador", "chat"], "solo los dos mas gordos");
    }

    #[test]
    fn lo_que_no_llega_al_umbral_ni_se_mira() {
        let processes = vec![process(10, "pequeno", 40), process(11, "minimo", 1)];
        let selection = select_throttle_targets(&processes, &config(), 1, None);
        assert!(selection.targets.is_empty());
    }

    #[test]
    fn nunca_se_toca_el_shell_ni_el_juego() {
        let processes = vec![process(1, "gamingmode", 300), process(42, "eljuego", 4000), process(10, "otro", 500)];
        let selection = select_throttle_targets(&processes, &config(), 1, Some(42));
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["otro"]);
        assert!(selection.untouched.iter().any(|u| u.name == "eljuego" && u.reason.contains("juego en marcha")));
    }

    #[test]
    fn las_utilidades_de_portatil_quedan_fuera_aunque_pesen() {
        let processes = vec![
            process(10, "HandheldCompanion", 900),
            process(11, "SteamController", 800),
            process(12, "navegador", 300),
        ];
        let selection = select_throttle_targets(&processes, &config(), 1, None);
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["navegador"], "los ayudantes del mando no se tocan");
        assert_eq!(selection.untouched.len(), 2);
        assert!(selection.untouched.iter().any(|u| u.reason.contains("Handheld Companion")));
    }

    #[test]
    fn los_procesos_criticos_de_windows_quedan_fuera() {
        let processes = vec![process(10, "svchost", 900), process(11, "lsass", 800), process(12, "app", 300)];
        let selection = select_throttle_targets(&processes, &config(), 1, None);
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["app"]);
    }

    #[test]
    fn el_informe_no_repite_nombres_protegidos() {
        let processes = (0..20).map(|i| process(100 + i, "svchost", 50)).collect::<Vec<_>>();
        let selection = select_throttle_targets(&processes, &config(), 1, None);
        assert_eq!(selection.untouched.len(), 1, "20 svchost, una sola linea");
    }

    #[test]
    fn los_forzados_entran_aunque_sean_ligeros_y_no_gastan_cupo() {
        let mut config = config();
        config.extra_throttle = vec!["manias".to_string()];
        let processes = vec![
            process(10, "manias", 3),
            process(11, "gordo1", 900),
            process(12, "gordo2", 800),
            process(13, "gordo3", 700),
        ];
        let selection = select_throttle_targets(&processes, &config, 1, None);
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["manias", "gordo1", "gordo2"], "el forzado no ocupa una de las 2 plazas");
        assert!(selection.targets[0].forced);
    }

    #[test]
    fn con_la_busqueda_automatica_apagada_solo_quedan_los_forzados() {
        let mut config = config();
        config.auto_throttle = false;
        config.extra_throttle = vec!["manias".to_string()];
        let processes = vec![process(10, "manias", 3), process(11, "gordo", 900)];
        let selection = select_throttle_targets(&processes, &config, 1, None);
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["manias"]);
    }

    #[test]
    fn un_proceso_hijo_del_juego_tambien_queda_protegido() {
        // 42 = lanzador (el PID que se guardo al arrancar), 43 = el juego de
        // verdad, que el lanzador arranco como hijo suyo. Patron comun con
        // Steam, instaladores o un .bat de por medio.
        let processes = vec![
            process(1, "gamingmode", 300),
            process(42, "lanzador", 50),
            child_process(43, 42, "eljuegodeverdad", 4000),
            process(10, "otro", 500),
        ];
        let selection = select_throttle_targets(&processes, &config(), 1, Some(42));
        let names: Vec<&str> = selection.targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["otro"], "el hijo del lanzador no debe degradarse");
        assert!(selection.untouched.iter().any(|u| u.name == "eljuegodeverdad"));
    }

    #[test]
    fn la_proteccion_de_descendencia_no_cuelga_con_una_cadena_larga() {
        // Cadena de 20 padres, mas larga que el limite de profundidad: no
        // debe colgarse, simplemente deja de reconocer el parentesco.
        let mut processes = vec![process(1, "gamingmode", 300)];
        for i in 0..20u32 {
            let parent = if i == 0 { 42 } else { 100 + i - 1 };
            processes.push(child_process(100 + i, parent, "eslabon", 10));
        }
        let selection = select_throttle_targets(&processes, &config(), 1, Some(42));
        // No debe entrar en panico ni bucle infinito; el resultado exacto
        // (protegido o no segun la profundidad) es secundario.
        assert!(selection.targets.len() + selection.untouched.len() > 0);
    }

    #[test]
    fn una_cadena_de_padres_con_ciclo_no_cuelga() {
        // Dos procesos que se apuntan mutuamente como padre: un dato corrupto
        // o una coincidencia de PIDs reciclados, no algo que deba darse en un
        // arbol de procesos real, pero tampoco debe colgar la busqueda.
        let processes =
            vec![process(1, "gamingmode", 300), child_process(10, 11, "a", 500), child_process(11, 10, "b", 500)];
        let selection = select_throttle_targets(&processes, &config(), 1, Some(999));
        assert_eq!(selection.targets.len(), 2, "sin relacion con el juego, se degradan como cualquier otro");
    }

    #[test]
    fn un_proceso_protegido_por_el_usuario_no_entra_ni_forzado() {
        let mut config = config();
        config.protected_processes = vec!["intocable".to_string()];
        config.extra_throttle = vec!["intocable".to_string()];
        let processes = vec![process(10, "intocable", 900)];
        let selection = select_throttle_targets(&processes, &config, 1, None);
        assert!(selection.targets.is_empty(), "proteger gana a degradar");
        assert_eq!(selection.untouched.len(), 1);
    }
}
