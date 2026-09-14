//! Dibujado. Cada vista vive en su propio modulo y todas comparten la barra
//! superior, la leyenda de botones de abajo y los avisos.

mod catalog;
mod cover_setup;
mod detail;
mod downloads;
mod explorer;
mod library;
mod quick;
mod resources;
mod settings;

pub use settings::SETTINGS_ROWS;

use egui::{Align, Align2, Color32, Frame, Margin, Rect, RichText, Rounding, Sense, Stroke, TextureHandle, Vec2};

use crate::app::{App, ToastKind, View};
use crate::theme;

impl App {
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.top_bar(ctx);
        self.bottom_bar(ctx);

        egui::CentralPanel::default()
            .frame(Frame::none().fill(theme::pal().bg).inner_margin(Margin::symmetric(28.0, 18.0)))
            .show(ctx, |ui| match self.view {
                View::Library => self.library_view_ui(ui),
                View::GameDetail => self.detail_ui(ui),
                View::Catalog => self.catalog_ui(ui),
                View::CatalogEntries => self.entries_ui(ui),
                View::Downloads => self.downloads_ui(ui),
                View::Resources => self.resources_ui(ui),
                View::Settings => self.settings_ui(ui),
                View::CoverSetup => self.cover_setup_ui(ui),
                View::Quick => self.quick_ui(ui),
                View::Browser => self.explorer_ui(ui),
            });

        self.toasts_ui(ctx);
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("barra_superior")
            .exact_height(66.0)
            .frame(Frame::none().fill(theme::pal().surface).inner_margin(Margin::symmetric(28.0, 12.0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("MODO JUEGO").size(22.0).strong().color(theme::pal().accent));
                    ui.add_space(18.0);
                    ui.label(RichText::new(self.view_title()).size(20.0).color(theme::pal().text_dim));

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        // Estado del optimizador.
                        let (label, color) = if self.power_busy() {
                            ("trabajando...", theme::pal().warn)
                        } else if self.power_engaged {
                            ("optimizado", theme::pal().good)
                        } else {
                            ("normal", theme::pal().text_dim)
                        };
                        chip(ui, label, color);

                        if self.memory.total > 0 {
                            let used = gm_core::util::format_bytes(self.memory.used());
                            let total = gm_core::util::format_bytes(self.memory.total);
                            let load = self.memory.used() as f32 / self.memory.total as f32;
                            let color = if load > 0.9 {
                                theme::pal().bad
                            } else if load > 0.75 {
                                theme::pal().warn
                            } else {
                                theme::pal().text_dim
                            };
                            chip(ui, &format!("RAM {used} / {total}"), color);
                        }

                        let pads = self.input.connected_pads();
                        let color = if pads > 0 { theme::pal().good } else { theme::pal().text_dim };
                        let label = match self.input.pad_kind() {
                            Some(kind) => {
                                format!(
                                    "{pads} mando(s) ({})",
                                    crate::nav::InputSource::from_pad_kind(kind).short_name()
                                )
                            }
                            None => format!("{pads} mando(s)"),
                        };
                        chip(ui, &label, color);

                        if let Some(session) = self.session.as_ref() {
                            chip(
                                ui,
                                &format!(
                                    "jugando: {} ({})",
                                    session.title,
                                    gm_core::util::format_playtime(session.elapsed_secs().max(60))
                                ),
                                theme::pal().accent,
                            );
                        }
                    });
                });
            });
    }

    fn bottom_bar(&mut self, ctx: &egui::Context) {
        use gm_input::NavAction;

        // Las acciones, no los nombres de boton: el glifo que se ensena sale
        // de `self.input_source`, que cambia solo segun con que se juega.
        let hints: &[(NavAction, &str)] = match self.view {
            View::Library => &[
                (NavAction::Accept, "Ver"),
                (NavAction::Back, "Limpiar busqueda"),
                (NavAction::Context, "Anadir .exe"),
                (NavAction::Favorite, "Favorito"),
                (NavAction::TabPrev, "Orden"),
                (NavAction::TabNext, "Catalogo"),
                (NavAction::Search, "Buscar"),
                (NavAction::Menu, "Menu"),
            ],
            View::GameDetail => &[
                (NavAction::Accept, "Jugar"),
                (NavAction::Back, "Volver"),
                (NavAction::Context, "Quitar"),
                (NavAction::Favorite, "Favorito"),
                (NavAction::Search, "Buscar caratula"),
            ],
            View::Catalog => &[
                (NavAction::Accept, "Abrir"),
                (NavAction::Back, "Volver"),
                (NavAction::Context, "Carpeta del catalogo"),
                (NavAction::TabNext, "Descargas"),
            ],
            View::CatalogEntries => &[
                (NavAction::Accept, "Descargar"),
                (NavAction::Back, "Volver"),
                (NavAction::Context, "Anadir ROM local"),
                (NavAction::Search, "Buscar"),
                (NavAction::TabNext, "Descargas"),
            ],
            View::Downloads => &[
                (NavAction::Back, "Volver"),
                (NavAction::Context, "Cancelar"),
                (NavAction::Favorite, "Limpiar terminadas"),
            ],
            View::Resources => &[
                (NavAction::Accept, "Marcar/desmarcar servicio"),
                (NavAction::Back, "Volver"),
                (NavAction::Context, "Volver a medir"),
                (NavAction::Favorite, "Seleccion recomendada"),
            ],
            View::Settings => &[
                (NavAction::Right, "Cambiar"),
                (NavAction::Left, "Valor anterior"),
                (NavAction::Back, "Guardar y volver"),
            ],
            View::CoverSetup => &[
                (NavAction::Accept, "Editar"),
                (NavAction::Context, "Pegar"),
                (NavAction::Favorite, "Probar clave"),
                (NavAction::TabPrev, "Borrar"),
                (NavAction::TabNext, "Descargar las que faltan"),
                (NavAction::Back, "Guardar y volver"),
            ],
            View::Quick => &[(NavAction::Accept, "Elegir"), (NavAction::Back, "Cerrar")],
            View::Browser => {
                &[(NavAction::Accept, "Abrir"), (NavAction::Back, "Subir"), (NavAction::Favorite, "Usar esta carpeta")]
            }
        };

        let source = self.input_source;
        egui::TopBottomPanel::bottom("barra_inferior")
            .exact_height(52.0)
            .frame(Frame::none().fill(theme::pal().surface).inner_margin(Margin::symmetric(28.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for (action, label) in hints {
                        button_hint(ui, crate::nav::button_label(source, *action), label);
                        ui.add_space(14.0);
                    }
                });
            });
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("avisos"))
            .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-28.0, -70.0))
            .interactable(false)
            .show(ctx, |ui| {
                for toast in &self.toasts {
                    let color = match toast.kind {
                        ToastKind::Info => theme::pal().accent,
                        ToastKind::Good => theme::pal().good,
                        ToastKind::Bad => theme::pal().bad,
                    };
                    Frame::none()
                        .fill(theme::pal().surface_alt)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0, color))
                        .inner_margin(Margin::symmetric(16.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_max_width(520.0);
                            ui.label(RichText::new(&toast.text).color(theme::pal().text));
                        });
                    ui.add_space(8.0);
                }
            });
    }

    fn view_title(&self) -> String {
        match self.view {
            View::Library => format!("Biblioteca ({})", self.library_view.len()),
            View::GameDetail => self.focused_game().map(|g| g.title.clone()).unwrap_or_default(),
            View::Catalog => format!("Catalogo ({} plataformas)", self.catalog.platforms().len()),
            View::CatalogEntries => {
                self.open_platform.as_deref().map(gm_catalog::display_name).unwrap_or_else(|| "Catalogo".to_string())
            }
            View::Downloads => "Descargas".to_string(),
            View::Resources => "Recursos del sistema".to_string(),
            View::Settings => "Ajustes".to_string(),
            View::CoverSetup => "Caratulas · SteamGridDB".to_string(),
            View::Quick => "Panel rapido".to_string(),
            View::Browser => "Explorador".to_string(),
        }
    }
}

// ----------------------------------------------------------------------
// Widgets compartidos
// ----------------------------------------------------------------------

pub fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    Frame::none()
        .fill(theme::pal().surface_alt)
        .rounding(Rounding::same(14.0))
        .inner_margin(Margin::symmetric(12.0, 5.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(15.0).color(color));
        });
    ui.add_space(10.0);
}

pub fn button_hint(ui: &mut egui::Ui, button: &str, label: &str) {
    Frame::none()
        .fill(theme::pal().accent_dim)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(9.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new(button).size(14.0).strong().color(theme::pal().text));
        });
    ui.add_space(6.0);
    ui.label(RichText::new(label).size(15.0).color(theme::pal().text_dim));
}

/// Boton grande y legible a distancia, para las acciones principales de una
/// pantalla (jugar, descargar, guardar...). El texto se pinta oscuro sobre el
/// acento y claro sobre los tonos de fondo.
pub fn action_button(ui: &mut egui::Ui, text: &str, fill: Color32) -> bool {
    // 210 de ancho: con 4 en fila (la ficha de un juego llega a tener esas)
    // siguen cabiendo en el ancho minimo de la ventana (960 px).
    let (rect, response) = ui.allocate_exact_size(Vec2::new(210.0, 54.0), Sense::click());
    ui.painter().rect_filled(rect, Rounding::same(10.0), fill);
    let text_color = if fill == theme::pal().accent { theme::pal().bg } else { theme::pal().text };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, egui::FontId::proportional(19.0), text_color);
    response.clicked()
}

/// Recuadro de foco: el usuario tiene que saber donde esta sin pensarlo.
pub fn focus_ring(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_stroke(
        rect.expand(3.0),
        Rounding::same(theme::CARD_ROUNDING + 3.0),
        Stroke::new(3.0, theme::pal().accent),
    );
}

/// Caratula de un juego. Si hay una imagen real (SteamGridDB) se dibuja
/// ella sola, tal cual viene, sin superponerle titulo ni iniciales: el arte ya
/// trae el logo del juego. Sin imagen, se cae en un recuadro generado.
pub fn cover_tile(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    subtitle: &str,
    focused: bool,
    texture: Option<&TextureHandle>,
) {
    match texture {
        Some(texture) => real_cover_tile(ui, rect, texture, focused),
        None => generated_cover_tile(ui, rect, title, subtitle, focused),
    }
}

/// Recorta la imagen para llenar el hueco sin deformarla ("cover", como en
/// CSS): el sobrante se pierde por los lados o por arriba/abajo, nunca se
/// encoge la imagen entera dejando bandas vacias.
fn real_cover_tile(ui: &mut egui::Ui, rect: Rect, texture: &TextureHandle, focused: bool) {
    let size = texture.size_vec2();
    let image_aspect = size.x / size.y;
    let rect_aspect = rect.width() / rect.height();

    let uv = if image_aspect > rect_aspect {
        let visible_fraction = rect_aspect / image_aspect;
        let margin = (1.0 - visible_fraction) / 2.0;
        Rect::from_min_max(egui::pos2(margin, 0.0), egui::pos2(1.0 - margin, 1.0))
    } else {
        let visible_fraction = image_aspect / rect_aspect;
        let margin = (1.0 - visible_fraction) / 2.0;
        Rect::from_min_max(egui::pos2(0.0, margin), egui::pos2(1.0, 1.0 - margin))
    };

    ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
    if focused {
        focus_ring(ui, rect);
    }
}

/// Caratula generada: color estable derivado del titulo e iniciales grandes.
/// Es lo que se ve mientras no hay arte real, o si no se encuentra ninguno.
fn generated_cover_tile(ui: &mut egui::Ui, rect: Rect, title: &str, subtitle: &str, focused: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(theme::CARD_ROUNDING), theme::shade_for(title));

    // Franja inferior para el texto, legible sobre cualquier tono.
    let strip = Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 76.0), rect.max);
    painter.rect_filled(
        strip,
        Rounding { nw: 0.0, ne: 0.0, sw: theme::CARD_ROUNDING, se: theme::CARD_ROUNDING },
        Color32::from_black_alpha(190),
    );

    painter.text(
        rect.center() - Vec2::new(0.0, 26.0),
        Align2::CENTER_CENTER,
        theme::initials(title),
        egui::FontId::proportional(58.0),
        Color32::from_white_alpha(210),
    );
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.bottom() - 52.0),
        Align2::LEFT_TOP,
        ellipsize(title, 22),
        egui::FontId::proportional(17.0),
        theme::pal().text,
    );
    if !subtitle.is_empty() {
        painter.text(
            egui::pos2(rect.left() + 12.0, rect.bottom() - 28.0),
            Align2::LEFT_TOP,
            ellipsize(subtitle, 26),
            egui::FontId::proportional(14.0),
            theme::pal().text_dim,
        );
    }
    if focused {
        focus_ring(ui, rect);
    }
}

/// Fila de lista con foco, usada en catalogo, descargas y explorador.
pub fn list_row(ui: &mut egui::Ui, focused: bool, height: f32, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let fill = if focused { theme::pal().accent_dim } else { theme::pal().surface };
    ui.painter().rect_filled(rect, Rounding::same(8.0), fill);
    if focused {
        ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(2.0, theme::pal().accent));
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(16.0, 8.0))).layout(egui::Layout::top_down(Align::Min)),
    );
    add(&mut child);
    response
}

pub fn ellipsize(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Devuelve true la primera vez que el foco cambia a `focus`, para desplazar la
/// lista solo entonces y no pelearse con la rueda del raton.
pub fn focus_changed(ctx: &egui::Context, key: &str, focus: usize) -> bool {
    let id = egui::Id::new(key);
    let previous: Option<usize> = ctx.data(|data| data.get_temp(id));
    if previous != Some(focus) {
        ctx.data_mut(|data| data.insert_temp(id, focus));
        true
    } else {
        false
    }
}

pub fn empty_state(ui: &mut egui::Ui, title: &str, hint: &str) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).size(26.0).color(theme::pal().text));
        ui.add_space(10.0);
        ui.label(RichText::new(hint).size(18.0).color(theme::pal().text_dim));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_recorte_de_texto_respeta_los_caracteres_multibyte() {
        assert_eq!(ellipsize("Pokemon", 20), "Pokemon");
        assert_eq!(ellipsize("Edición Española Larga", 10), "Edición E…");
    }
}
