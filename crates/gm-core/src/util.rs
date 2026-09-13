use std::io::Write;
use std::path::Path;

use crate::error::Result;

/// Escribe a un temporal y renombra. Evita dejar `library.json` a medias si se
/// corta la luz en mitad de una partida, que es exactamente cuando pasa.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    // En Windows rename falla si el destino existe.
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// FNV-1a de 64 bits: identificadores estables sin arrastrar una dependencia
/// de uuid solo para esto.
pub fn stable_id(parts: &[&str]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Normaliza un titulo para buscar: minusculas, sin acentos y sin puntuacion.
/// Los separadores se colapsan en un unico espacio.
pub fn normalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        let ch = fold_accent(ch);
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            pending_space = true;
        }
    }
    out
}

fn fold_accent(ch: char) -> char {
    match ch {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        other => other,
    }
}

/// Formatea segundos de juego como "12 h 34 min" / "5 min".
pub fn format_playtime(seconds: u64) -> String {
    if seconds < 60 {
        return "sin jugar".to_string();
    }
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours} h {minutes} min")
    } else {
        format!("{minutes} min")
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_acentos_y_puntuacion() {
        assert_eq!(normalize("Pokémon: Edición Roja"), "pokemon edicion roja");
        assert_eq!(normalize("  F-ZERO  (USA) "), "f zero usa");
        assert_eq!(normalize("Ninõ"), "nino");
    }

    #[test]
    fn los_ids_son_estables_y_distintos() {
        let a = stable_id(&["C:/juegos/a.exe", "Juego A"]);
        assert_eq!(a, stable_id(&["C:/juegos/a.exe", "Juego A"]));
        assert_ne!(a, stable_id(&["C:/juegos/b.exe", "Juego A"]));
        // Un id no debe poder falsificarse concatenando los campos.
        assert_ne!(stable_id(&["ab", "c"]), stable_id(&["a", "bc"]));
    }

    #[test]
    fn formatea_tiempos_y_tamanos() {
        assert_eq!(format_playtime(30), "sin jugar");
        assert_eq!(format_playtime(3600 * 2 + 60 * 5), "2 h 5 min");
        assert_eq!(format_bytes(2048), "2.0 KB");
    }
}
