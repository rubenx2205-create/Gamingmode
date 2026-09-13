//! GUIDs de planes de energia, con parseo propio para poder probarlo fuera de
//! Windows y guardarlos como texto en el snapshot.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemeGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl SchemeGuid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('{').trim_end_matches('}');
        let parts: Vec<&str> = text.split('-').collect();
        if parts.len() != 5
            || parts[0].len() != 8
            || parts[1].len() != 4
            || parts[2].len() != 4
            || parts[3].len() != 4
            || parts[4].len() != 12
        {
            return None;
        }
        let data1 = u32::from_str_radix(parts[0], 16).ok()?;
        let data2 = u16::from_str_radix(parts[1], 16).ok()?;
        let data3 = u16::from_str_radix(parts[2], 16).ok()?;
        let mut data4 = [0u8; 8];
        let tail: String = format!("{}{}", parts[3], parts[4]);
        for (i, slot) in data4.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&tail[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self { data1, data2, data3, data4 })
    }
}

impl std::fmt::Display for SchemeGuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7]
        )
    }
}

/// Planes de energia integrados en Windows.
pub const BALANCED: SchemeGuid =
    SchemeGuid::new(0x381b4222, 0xf694, 0x41f0, [0x96, 0x85, 0xff, 0x5b, 0xb2, 0x60, 0xdf, 0x2e]);
pub const HIGH_PERFORMANCE: SchemeGuid =
    SchemeGuid::new(0x8c5e7fda, 0xe8bf, 0x4a96, [0x9a, 0x85, 0xa6, 0xe2, 0x3a, 0x8c, 0x63, 0x5c]);
/// "Maximo rendimiento": oculto por defecto y ausente en portatiles, hay que
/// duplicarlo con `powercfg -duplicatescheme` antes de poder activarlo.
pub const ULTIMATE: SchemeGuid =
    SchemeGuid::new(0xe9a42b02, 0xd5df, 0x448d, [0xaa, 0x00, 0x03, 0xf1, 0x47, 0x49, 0xeb, 0x61]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_de_texto() {
        let text = "e9a42b02-d5df-448d-aa00-03f14749eb61";
        let guid = SchemeGuid::parse(text).unwrap();
        assert_eq!(guid, ULTIMATE);
        assert_eq!(guid.to_string(), text);
    }

    #[test]
    fn acepta_llaves_y_rechaza_basura() {
        assert_eq!(SchemeGuid::parse("{381b4222-f694-41f0-9685-ff5bb260df2e}"), Some(BALANCED));
        assert_eq!(SchemeGuid::parse("no-soy-un-guid"), None);
        assert_eq!(SchemeGuid::parse(""), None);
    }
}
