//! Caratulas reales via SteamGridDB (https://www.steamgriddb.com).
//!
//! Es una base de datos comunitaria de arte de caratulas, no una tienda: no
//! hace falta cuenta para jugar ni se lanza nada desde aqui. Solo se busca el
//! juego por titulo, se descarga la imagen mejor puntuada y se guarda en
//! disco. Sin clave configurada, este modulo no hace nada.
//!
//! Todo pasa en un hilo aparte (ver `gm_shell::app`): la busqueda y la
//! descarga tardan del orden de un segundo, y no pueden bloquear el bucle de
//! dibujado ni sacar al shell de pantalla completa para nada (ni una ventana
//! de navegador, ni un dialogo de inicio de sesion: la clave se guarda una
//! vez en `config.toml` y a partir de ahi es silencioso).

use std::path::{Path, PathBuf};

use gm_core::error::{Error, Result};

const API_BASE: &str = "https://www.steamgriddb.com/api/v2";

/// Busca la caratula de un juego y la descarga a `dest_dir/{game_id}.{ext}`.
/// Devuelve la ruta final si encuentra algo.
#[cfg(windows)]
pub fn fetch_cover(api_key: &str, title: &str, game_id: &str, dest_dir: &Path) -> Result<PathBuf> {
    let agent = build_agent()?;

    let steamgrid_id = search_game(&agent, api_key, title)?;
    let image_url = best_grid_url(&agent, api_key, steamgrid_id)?;
    download_image(&agent, &image_url, game_id, dest_dir)
}

#[cfg(not(windows))]
pub fn fetch_cover(_api_key: &str, _title: &str, _game_id: &str, _dest_dir: &Path) -> Result<PathBuf> {
    Err(Error::Unsupported("las caratulas de SteamGridDB solo se descargan en Windows"))
}

/// Comprueba que una clave funciona de verdad, sin descargar nada: solo
/// pregunta por un titulo archiconocido y mira si SteamGridDB acepta la
/// clave. Es lo que hay detras del boton "Probar clave" de los ajustes: la
/// alternativa -guardarla a ciegas y enterarte de que estaba mal cuando falla
/// en silencio la primera busqueda de verdad- no es nada entendible.
#[cfg(windows)]
pub fn validate_key(api_key: &str) -> Result<()> {
    let agent = build_agent()?;
    let url = format!("{API_BASE}/search/autocomplete/half-life");
    match agent.get(&url).set("Authorization", &format!("Bearer {}", api_key.trim())).call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => {
            Err(Error::Remote("SteamGridDB ha rechazado la clave (401/403): copiala entera, sin espacios".into()))
        }
        Err(e) => Err(Error::Remote(format!("no se pudo contactar con SteamGridDB: {e}"))),
    }
}

#[cfg(not(windows))]
pub fn validate_key(_api_key: &str) -> Result<()> {
    Err(Error::Unsupported("la validacion de la clave solo funciona en Windows"))
}

#[cfg(windows)]
fn build_agent() -> Result<ureq::Agent> {
    crate::download::build_agent().ok_or_else(|| Error::Remote("no se pudo iniciar el transporte TLS".into()))
}

/// Primer resultado de autocompletado para el titulo dado.
#[cfg(windows)]
fn search_game(agent: &ureq::Agent, api_key: &str, title: &str) -> Result<u64> {
    let url = format!("{API_BASE}/search/autocomplete/{}", percent_encode(title));
    let body: serde_json::Value = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {}", api_key.trim()))
        .call()
        .map_err(|e| Error::Remote(format!("busqueda en SteamGridDB: {e}")))?
        .into_json()
        .map_err(|e| Error::Remote(format!("respuesta de SteamGridDB ilegible: {e}")))?;

    body["data"]
        .get(0)
        .and_then(|entry| entry["id"].as_u64())
        .ok_or_else(|| Error::NotFound(format!("ningun juego de SteamGridDB coincide con '{title}'")))
}

/// URL de la caratula mejor puntuada para ese id de SteamGridDB. Se piden
/// dimensiones verticales (las de una caja de juego) y solo imagenes
/// estaticas, para no complicar la decodificacion en la interfaz.
#[cfg(windows)]
fn best_grid_url(agent: &ureq::Agent, api_key: &str, steamgrid_id: u64) -> Result<String> {
    let url = format!("{API_BASE}/grids/game/{steamgrid_id}?dimensions=600x900,342x482&types=static");
    let body: serde_json::Value = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {}", api_key.trim()))
        .call()
        .map_err(|e| Error::Remote(format!("caratulas de SteamGridDB: {e}")))?
        .into_json()
        .map_err(|e| Error::Remote(format!("respuesta de SteamGridDB ilegible: {e}")))?;

    body["data"]
        .get(0)
        .and_then(|entry| entry["url"].as_str())
        .map(str::to_string)
        .ok_or_else(|| Error::NotFound("SteamGridDB no tiene ninguna caratula para ese juego".to_string()))
}

#[cfg(windows)]
fn download_image(agent: &ureq::Agent, url: &str, game_id: &str, dest_dir: &Path) -> Result<PathBuf> {
    use std::io::Read;

    let response = agent.get(url).call().map_err(|e| Error::Remote(format!("descarga de caratula: {e}")))?;
    let ext = url.rsplit('.').next().filter(|ext| ext.len() <= 4).unwrap_or("png");

    std::fs::create_dir_all(dest_dir)?;
    let dest = dest_dir.join(format!("{game_id}.{ext}"));

    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes)?;
    gm_core::util::write_atomic(&dest, &bytes)?;
    Ok(dest)
}

/// Codificacion minima para un termino de busqueda: espacios y puntuacion
/// habitual en titulos de videojuegos. No pretende ser un codificador de URL
/// completo, solo lo justo para que un titulo con espacios o dos puntos no
/// rompa la peticion.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_codificacion_deja_intactos_los_caracteres_seguros() {
        assert_eq!(percent_encode("Doom"), "Doom");
        assert_eq!(percent_encode("Super Mario World"), "Super%20Mario%20World");
        assert_eq!(percent_encode("Pokemon: Red"), "Pokemon%3A%20Red");
    }

    #[cfg(not(windows))]
    #[test]
    fn fuera_de_windows_no_se_intenta_descargar_nada() {
        let dir = std::env::temp_dir().join(format!("gm-steamgrid-{}", std::process::id()));
        let result = fetch_cover("clave", "Doom", "abc123", &dir);
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn fuera_de_windows_tampoco_se_valida_ninguna_clave() {
        assert!(matches!(validate_key("clave"), Err(Error::Unsupported(_))));
    }
}
