//! Navegacion: teclado y mando hablan el mismo idioma (`NavAction`) y la
//! rejilla se mueve con aritmetica pura, facil de probar.

use gm_input::{NavAction, PadKind};

/// Con que se esta jugando ahora mismo, a efectos de que boton ensenar en
/// pantalla. Cambia solo: se recalcula cada frame segun de donde vino la
/// ultima pulsacion, igual que hace Steam o cualquier consola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Xbox,
    PlayStation,
    KeyboardMouse,
}

impl InputSource {
    pub fn from_pad_kind(kind: PadKind) -> Self {
        match kind {
            PadKind::Xbox => InputSource::Xbox,
            PadKind::PlayStation => InputSource::PlayStation,
        }
    }

    /// Nombre corto para la cabecera ("1 mando (PlayStation)").
    pub fn short_name(self) -> &'static str {
        match self {
            InputSource::Xbox => "Xbox",
            InputSource::PlayStation => "PlayStation",
            InputSource::KeyboardMouse => "teclado",
        }
    }
}

/// Hubo alguna pulsacion de teclado (o clic de raton) este frame? Cualquier
/// tecla cuenta, no solo las que forman parte de `keyboard_actions`: escribir
/// "z" en el buscador no es una accion de navegacion, pero sigue siendo
/// evidencia de que se esta jugando con teclado y no con mando.
pub fn keyboard_or_mouse_activity(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input.pointer.any_click()
            || input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)))
    })
}

/// Como se llama, en esta fuente, el boton que dispara `action`.
///
/// Las direcciones usan siempre el mismo glifo neutro: cruceta, stick y
/// flechas son la misma idea con otra forma, y no hace falta distinguir marca
/// para ellas. Los botones de cara si cambian: XInput normaliza cualquier
/// mando al esquema A/B/X/Y, pero un DualSense fisico no tiene esas letras
/// serigrafiadas.
pub fn button_label(source: InputSource, action: NavAction) -> &'static str {
    match action {
        NavAction::Up => "▲",
        NavAction::Down => "▼",
        NavAction::Left => "◀",
        NavAction::Right => "▶",
        _ => match source {
            InputSource::KeyboardMouse => keyboard_label(action),
            InputSource::Xbox => xbox_label(action),
            InputSource::PlayStation => playstation_label(action),
        },
    }
}

fn xbox_label(action: NavAction) -> &'static str {
    match action {
        NavAction::Accept => "A",
        NavAction::Back => "B",
        NavAction::Context => "X",
        NavAction::Favorite => "Y",
        NavAction::Menu => "Start",
        NavAction::Search => "Select",
        NavAction::TabPrev => "LB",
        NavAction::TabNext => "RB",
        NavAction::PageUp => "LT",
        NavAction::PageDown => "RT",
        NavAction::Guide => "Guia",
        NavAction::Up | NavAction::Down | NavAction::Left | NavAction::Right => unreachable!("filtrado arriba"),
    }
}

fn playstation_label(action: NavAction) -> &'static str {
    match action {
        NavAction::Accept => "✕",
        NavAction::Back => "○",
        NavAction::Context => "□",
        NavAction::Favorite => "△",
        NavAction::Menu => "Options",
        NavAction::Search => "Share",
        NavAction::TabPrev => "L1",
        NavAction::TabNext => "R1",
        NavAction::PageUp => "L2",
        NavAction::PageDown => "R2",
        NavAction::Guide => "PS",
        NavAction::Up | NavAction::Down | NavAction::Left | NavAction::Right => unreachable!("filtrado arriba"),
    }
}

fn keyboard_label(action: NavAction) -> &'static str {
    match action {
        NavAction::Accept => "Enter",
        NavAction::Back => "Esc",
        NavAction::Context => "F2",
        NavAction::Favorite => "F",
        NavAction::Menu => "F1",
        NavAction::Search => "/",
        NavAction::TabPrev => "Mayus+Tab",
        NavAction::TabNext => "Tab",
        NavAction::PageUp => "RePag",
        NavAction::PageDown => "AvPag",
        NavAction::Guide => "F12",
        NavAction::Up | NavAction::Down | NavAction::Left | NavAction::Right => unreachable!("filtrado arriba"),
    }
}

/// Traduce las teclas pulsadas este frame a acciones de navegacion.
///
/// La auto-repeticion del teclado la da el sistema operativo, asi que aqui no
/// hay que replicar el temporizador del mando.
pub fn keyboard_actions(ctx: &egui::Context) -> Vec<NavAction> {
    use egui::Key;

    const BINDINGS: &[(Key, NavAction)] = &[
        (Key::ArrowUp, NavAction::Up),
        (Key::W, NavAction::Up),
        (Key::ArrowDown, NavAction::Down),
        (Key::S, NavAction::Down),
        (Key::ArrowLeft, NavAction::Left),
        (Key::A, NavAction::Left),
        (Key::ArrowRight, NavAction::Right),
        (Key::D, NavAction::Right),
        (Key::Enter, NavAction::Accept),
        (Key::Space, NavAction::Accept),
        (Key::Escape, NavAction::Back),
        (Key::Backspace, NavAction::Back),
        (Key::F2, NavAction::Context),
        (Key::F, NavAction::Favorite),
        (Key::F1, NavAction::Menu),
        (Key::Tab, NavAction::TabNext),
        (Key::PageUp, NavAction::PageUp),
        (Key::PageDown, NavAction::PageDown),
        (Key::F12, NavAction::Guide),
        // F11 (pantalla completa) lo trata el bucle principal: no es una
        // accion de navegacion.
    ];

    ctx.input(|input| {
        let mut actions = Vec::new();
        for (key, action) in BINDINGS {
            if input.key_pressed(*key) {
                // Shift+Tab retrocede de pestana.
                if *key == egui::Key::Tab && input.modifiers.shift {
                    actions.push(NavAction::TabPrev);
                } else {
                    actions.push(*action);
                }
            }
        }
        if input.key_pressed(egui::Key::Slash) || (input.modifiers.ctrl && input.key_pressed(egui::Key::F)) {
            actions.push(NavAction::Search);
        }
        actions
    })
}

/// Mueve el foco dentro de una rejilla de `columns` columnas.
///
/// El movimiento lateral no salta de fila: en una rejilla de caratulas, que el
/// cursor se teletransporte al otro extremo de la pantalla desorienta.
pub fn move_focus(current: usize, len: usize, columns: usize, action: NavAction) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    let columns = columns.max(1);
    let current = current.min(last);
    let page = columns * 3;

    match action {
        // Desde la primera fila no se sube: saltar a otra columna (que es lo
        // que haria una resta saturada) desorienta.
        NavAction::Up => {
            if current < columns {
                current
            } else {
                current - columns
            }
        }
        NavAction::Down => {
            let next = current + columns;
            // Desde la ultima fila incompleta se baja al ultimo elemento.
            if next <= last {
                next
            } else if current / columns < last / columns {
                last
            } else {
                current
            }
        }
        NavAction::Left => {
            if current % columns == 0 {
                current
            } else {
                current - 1
            }
        }
        NavAction::Right => {
            if current % columns == columns - 1 || current == last {
                current
            } else {
                current + 1
            }
        }
        NavAction::PageUp => {
            let target = current.saturating_sub(page);
            // Mantener la columna: se sube de tres en tres filas completas.
            if current < columns {
                current
            } else {
                target
            }
        }
        NavAction::PageDown => (current + page).min(last),
        _ => current,
    }
}

/// Movimiento en una lista vertical simple, con vuelta por los extremos.
pub fn move_list(current: usize, len: usize, action: NavAction) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    let current = current.min(last);
    match action {
        NavAction::Up => {
            if current == 0 {
                last
            } else {
                current - 1
            }
        }
        NavAction::Down => {
            if current == last {
                0
            } else {
                current + 1
            }
        }
        NavAction::PageUp => current.saturating_sub(5),
        NavAction::PageDown => (current + 5).min(last),
        _ => current,
    }
}

/// Cuantas columnas caben en el ancho disponible.
pub fn columns_for(available_width: f32, card_width: f32, spacing: f32) -> usize {
    let step = card_width + spacing;
    if step <= 0.0 {
        return 1;
    }
    (((available_width + spacing) / step).floor() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_rejilla_no_salta_de_fila_al_moverse_de_lado() {
        // 0 1 2
        // 3 4 5
        assert_eq!(move_focus(0, 6, 3, NavAction::Left), 0, "no retrocede desde el borde");
        assert_eq!(move_focus(2, 6, 3, NavAction::Right), 2, "no avanza desde el borde");
        assert_eq!(move_focus(3, 6, 3, NavAction::Left), 3);
        assert_eq!(move_focus(1, 6, 3, NavAction::Right), 2);
    }

    #[test]
    fn la_rejilla_sube_y_baja_por_columnas() {
        assert_eq!(move_focus(4, 6, 3, NavAction::Up), 1);
        assert_eq!(move_focus(1, 6, 3, NavAction::Down), 4);
        assert_eq!(move_focus(1, 6, 3, NavAction::Up), 1, "en la primera fila no pasa nada");
        assert_eq!(move_focus(2, 6, 3, NavAction::Up), 2, "tampoco cambia de columna al subir");
    }

    #[test]
    fn bajar_desde_una_fila_incompleta_lleva_al_ultimo() {
        // 0 1 2
        // 3 4
        assert_eq!(move_focus(2, 5, 3, NavAction::Down), 4, "no hay hueco debajo: al ultimo");
        assert_eq!(move_focus(4, 5, 3, NavAction::Down), 4, "en la ultima fila se queda");
    }

    #[test]
    fn la_rejilla_vacia_no_desborda() {
        assert_eq!(move_focus(0, 0, 3, NavAction::Down), 0);
        assert_eq!(move_focus(99, 3, 3, NavAction::Right), 2);
    }

    #[test]
    fn las_listas_dan_la_vuelta() {
        assert_eq!(move_list(0, 4, NavAction::Up), 3);
        assert_eq!(move_list(3, 4, NavAction::Down), 0);
        assert_eq!(move_list(0, 0, NavAction::Down), 0);
    }

    #[test]
    fn las_columnas_se_calculan_con_el_espaciado() {
        // 3 tarjetas de 190 + 2 huecos de 14 = 598 <= 600
        assert_eq!(columns_for(600.0, 190.0, 14.0), 3);
        assert_eq!(columns_for(100.0, 190.0, 14.0), 1, "siempre al menos una");
    }

    #[test]
    fn cada_fuente_tiene_su_propio_glifo_para_aceptar() {
        assert_eq!(button_label(InputSource::Xbox, NavAction::Accept), "A");
        assert_eq!(button_label(InputSource::PlayStation, NavAction::Accept), "✕");
        assert_eq!(button_label(InputSource::KeyboardMouse, NavAction::Accept), "Enter");
    }

    #[test]
    fn las_direcciones_no_dependen_de_la_fuente() {
        for source in [InputSource::Xbox, InputSource::PlayStation, InputSource::KeyboardMouse] {
            assert_eq!(button_label(source, NavAction::Up), "▲");
            assert_eq!(button_label(source, NavAction::Right), "▶");
        }
    }

    #[test]
    fn la_marca_del_mando_se_traduce_a_fuente_de_entrada() {
        assert_eq!(InputSource::from_pad_kind(PadKind::Xbox), InputSource::Xbox);
        assert_eq!(InputSource::from_pad_kind(PadKind::PlayStation), InputSource::PlayStation);
    }
}
