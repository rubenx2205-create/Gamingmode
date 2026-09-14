//! Alta de la clave de SteamGridDB: explicacion clara, campo para pegar o
//! escribir, y una prueba real contra el servicio antes de guardar nada.
//!
//! El objetivo es que nadie tenga que adivinar que es esto ni salir a editar
//! un fichero de texto: todo se hace desde aqui, con el mando o el teclado.

use egui::{Align, Frame, Margin, RichText, Rounding};
use gm_input::NavAction;

use super::action_button;
use crate::app::{App, KeyTestState};
use crate::nav::button_label;
use crate::theme;

impl App {
    pub(super) fn cover_setup_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Caratulas automaticas").size(30.0).strong());
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "SteamGridDB es una base de datos comunitaria de caratulas de videojuegos. \
                 No es una tienda: no hace falta cuenta para jugar, no se compra nada y no \
                 se lanza nada desde aqui. Con una clave gratuita, el modo juego busca solo \
                 la caratula de cada juego que aparece en la biblioteca.",
            )
            .size(17.0)
            .color(theme::pal().text_dim),
        );
        ui.add_space(18.0);

        self.cover_setup_steps(ui);
        ui.add_space(18.0);
        self.cover_setup_field(ui);
        ui.add_space(14.0);
        self.cover_setup_status(ui);
        ui.add_space(18.0);
        self.cover_setup_bulk_download(ui);
    }

    /// El boton que de verdad descarga arte, no solo lo configura: mientras
    /// no se pulse aqui (o se abra cada ficha con Buscar caratula), una clave
    /// guardada no baja nada por si sola salvo lo que se vaya viendo en la
    /// biblioteca al navegar.
    fn cover_setup_bulk_download(&mut self, ui: &mut egui::Ui) {
        let missing = self.library.games.iter().filter(|g| g.cover.is_none()).count();
        let has_key = self.config.covers.steamgrid_api_key.as_deref().is_some_and(|k| !k.trim().is_empty());

        Frame::none().fill(theme::pal().surface).rounding(Rounding::same(10.0)).inner_margin(Margin::same(16.0)).show(
            ui,
            |ui| {
                ui.label(RichText::new("Descargar ahora").size(17.0).strong());
                ui.add_space(6.0);
                ui.label(
                    RichText::new(if missing == 0 {
                        "Todos los juegos de la biblioteca ya tienen caratula.".to_string()
                    } else {
                        format!(
                            "{missing} juego(s) sin caratula todavia (incluidos los anadidos como .exe). \
                             La busqueda automatica solo se dispara al pasar por la biblioteca; este \
                             boton las busca todas de golpe, ahora mismo.",
                        )
                    })
                    .size(15.0)
                    .color(theme::pal().text_dim),
                );
                ui.add_space(10.0);

                let source = self.input_source;
                let label = format!("Descargar las que faltan  ({})", button_label(source, NavAction::TabNext));
                let fill = if has_key && missing > 0 { theme::pal().accent } else { theme::pal().surface_alt };
                if action_button(ui, &label, fill) {
                    // El propio metodo avisa si falta la clave: pulsar sin
                    // ella no se queda callado.
                    self.fetch_all_missing_covers();
                }
                if !has_key {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Guarda una clave arriba antes de poder descargar nada.")
                            .size(14.0)
                            .color(theme::pal().warn),
                    );
                }
            },
        );
    }

    fn cover_setup_steps(&self, ui: &mut egui::Ui) {
        Frame::none().fill(theme::pal().surface).rounding(Rounding::same(10.0)).inner_margin(Margin::same(16.0)).show(
            ui,
            |ui| {
                ui.label(RichText::new("Como conseguir una clave gratuita").size(17.0).strong());
                ui.add_space(8.0);
                for (n, step) in [
                    "Entra en steamgriddb.com y crea una cuenta (es gratis).",
                    "Ve a Preferencias -> API y pulsa \"Generate API key\".",
                    "Copia la clave y vuelve aqui: pegala con X, o escribela con el teclado.",
                ]
                .into_iter()
                .enumerate()
                {
                    ui.label(RichText::new(format!("{}. {step}", n + 1)).size(16.0).color(theme::pal().text));
                    ui.add_space(4.0);
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("steamgriddb.com/profile/preferences/api").size(15.0).color(theme::pal().accent),
                );
            },
        );
    }

    fn cover_setup_field(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Clave de API").size(16.0).color(theme::pal().text_dim));
        ui.add_space(6.0);

        let editing = self.cover_key_editing;
        Frame::none()
            .fill(theme::pal().surface_alt)
            .rounding(Rounding::same(8.0))
            .stroke(egui::Stroke::new(if editing { 2.0 } else { 1.0 }, theme::pal().accent))
            .inner_margin(Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.set_min_width(480.0);
                if editing {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.cover_key_draft)
                            .hint_text("Pega o escribe la clave aqui...")
                            .frame(false)
                            .font(egui::FontId::monospace(18.0))
                            .desired_width(f32::INFINITY),
                    );
                    if !response.has_focus() {
                        response.request_focus();
                    }
                    if response.changed() {
                        self.cover_key_test = KeyTestState::Idle;
                    }
                } else {
                    let shown = mask_key(&self.cover_key_draft);
                    let color = if self.cover_key_draft.is_empty() { theme::pal().text_dim } else { theme::pal().text };
                    ui.label(RichText::new(shown).size(18.0).monospace().color(color));
                }
            });
    }

    fn cover_setup_status(&self, ui: &mut egui::Ui) {
        let (text, color) = match &self.cover_key_test {
            KeyTestState::Idle if self.config.covers.steamgrid_api_key.is_some() => {
                ("Clave guardada. Pulsa Y para volver a probarla si algo falla.".to_string(), theme::pal().text_dim)
            }
            KeyTestState::Idle => {
                ("Sin guardar todavia. Pulsa Y para probarla antes de guardar.".to_string(), theme::pal().text_dim)
            }
            KeyTestState::Testing => ("Probando contra SteamGridDB...".to_string(), theme::pal().warn),
            KeyTestState::Valid => ("✔ La clave funciona.".to_string(), theme::pal().good),
            KeyTestState::Invalid(e) => (format!("✖ {e}"), theme::pal().bad),
        };
        ui.label(RichText::new(text).size(16.0).color(color));

        if !cfg!(windows) {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Probar la clave y descargar caratulas solo funciona en Windows.")
                    .size(14.0)
                    .color(theme::pal().text_dim),
            );
        }

        let _ = Align::Center;
    }
}

/// Ensena solo el principio y el final de la clave: lo justo para reconocerla
/// sin dejarla entera a la vista en una captura de pantalla o una demo.
fn mask_key(key: &str) -> String {
    if key.is_empty() {
        return "(sin configurar)".to_string();
    }
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "•".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}···{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_clave_vacia_se_marca_como_sin_configurar() {
        assert_eq!(mask_key(""), "(sin configurar)");
    }

    #[test]
    fn una_clave_corta_se_oculta_entera() {
        assert_eq!(mask_key("abc123"), "••••••");
    }

    #[test]
    fn una_clave_larga_ensena_solo_las_puntas() {
        assert_eq!(mask_key("sgdb-1234567890abcdef"), "sgdb···cdef");
    }
}
