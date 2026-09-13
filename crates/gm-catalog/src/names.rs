//! Nombres legibles y agrupacion por familia para los identificadores de
//! plataforma del repositorio Roms (el nombre del `.jsonl`).

/// Familia a la que pertenece una plataforma; sirve para agrupar la rejilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Nintendo,
    Sega,
    Sony,
    Microsoft,
    Arcade,
    Retro,
    Pc,
    Otros,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Nintendo => "Nintendo",
            Family::Sega => "Sega",
            Family::Sony => "Sony",
            Family::Microsoft => "Microsoft",
            Family::Arcade => "Arcade",
            Family::Retro => "Microordenadores",
            Family::Pc => "PC",
            Family::Otros => "Otros",
        }
    }

    pub const ALL: [Family; 8] = [
        Family::Nintendo,
        Family::Sega,
        Family::Sony,
        Family::Microsoft,
        Family::Arcade,
        Family::Retro,
        Family::Pc,
        Family::Otros,
    ];
}

/// Tabla ordenada por id para poder buscar en binario.
const TABLE: &[(&str, &str, Family)] = &[
    ("3do", "3DO Interactive", Family::Otros),
    ("3ds", "Nintendo 3DS", Family::Nintendo),
    ("acorn", "Acorn Electron", Family::Retro),
    ("adam", "Coleco Adam", Family::Retro),
    ("advision", "Adventure Vision", Family::Otros),
    ("amiga1200", "Amiga 1200", Family::Retro),
    ("amiga500", "Amiga 500", Family::Retro),
    ("amigacd32", "Amiga CD32", Family::Retro),
    ("amigacdtv", "Amiga CDTV", Family::Retro),
    ("amstradcpc", "Amstrad CPC", Family::Retro),
    ("apfm1000", "APF M1000", Family::Otros),
    ("apple2", "Apple II", Family::Retro),
    ("arcade", "Arcade", Family::Arcade),
    ("arcadia", "Arcadia 2001", Family::Otros),
    ("arduboy", "Arduboy", Family::Otros),
    ("astrocade", "Bally Astrocade", Family::Otros),
    ("atari2600", "Atari 2600", Family::Retro),
    ("atari5200", "Atari 5200", Family::Retro),
    ("atari7800", "Atari 7800", Family::Retro),
    ("atari800", "Atari 800", Family::Retro),
    ("atarist", "Atari ST", Family::Retro),
    ("atomiswave", "Atomiswave", Family::Arcade),
    ("bbcmicro", "BBC Micro", Family::Retro),
    ("c128", "Commodore 128", Family::Retro),
    ("c20", "Commodore VIC-20", Family::Retro),
    ("c64", "Commodore 64", Family::Retro),
    ("camplynx", "Camputers Lynx", Family::Retro),
    ("casloopy", "Casio Loopy", Family::Otros),
    ("cave", "CAVE", Family::Arcade),
    ("cdi", "Philips CD-i", Family::Otros),
    ("channelf", "Fairchild Channel F", Family::Otros),
    ("colecovision", "ColecoVision", Family::Retro),
    ("cplus4", "Commodore Plus/4", Family::Retro),
    ("cps", "Capcom CPS", Family::Arcade),
    ("cps1", "Capcom CPS-1", Family::Arcade),
    ("cps2", "Capcom CPS-2", Family::Arcade),
    ("cps3", "Capcom CPS-3", Family::Arcade),
    ("crvision", "CreatiVision", Family::Otros),
    ("dos", "MS-DOS", Family::Pc),
    ("dreamcast", "Sega Dreamcast", Family::Sega),
    ("easyrpg", "EasyRPG", Family::Otros),
    ("fbneo", "FinalBurn Neo", Family::Arcade),
    ("fds", "Famicom Disk System", Family::Nintendo),
    ("flash", "Flash", Family::Pc),
    ("fm7", "Fujitsu FM-7", Family::Retro),
    ("fmtowns", "FM Towns", Family::Retro),
    ("fpinball", "Future Pinball", Family::Pc),
    ("gamate", "Gamate", Family::Otros),
    ("gameandwatch", "Game & Watch", Family::Nintendo),
    ("gamecube", "Nintendo GameCube", Family::Nintendo),
    ("gamegear", "Sega Game Gear", Family::Sega),
    ("gamepock", "Game Pocket Computer", Family::Otros),
    ("gb", "Game Boy", Family::Nintendo),
    ("gba", "Game Boy Advance", Family::Nintendo),
    ("gbc", "Game Boy Color", Family::Nintendo),
    ("gmaster", "Game Master", Family::Otros),
    ("gp32", "GP32", Family::Otros),
    ("gx4000", "Amstrad GX4000", Family::Retro),
    ("intellivision", "Intellivision", Family::Retro),
    ("jaguar", "Atari Jaguar", Family::Retro),
    ("jaguarcd", "Atari Jaguar CD", Family::Retro),
    ("lowresnx", "LowRes NX", Family::Otros),
    ("lutro", "Lutro", Family::Otros),
    ("lynx", "Atari Lynx", Family::Retro),
    ("mame", "MAME", Family::Arcade),
    ("mastersystem", "Sega Master System", Family::Sega),
    ("megadrive", "Sega Mega Drive", Family::Sega),
    ("megadrive-msu", "Mega Drive MSU", Family::Sega),
    ("megaduck", "Mega Duck", Family::Otros),
    ("model2", "Sega Model 2", Family::Arcade),
    ("model3", "Sega Model 3", Family::Arcade),
    ("ms2", "MSX2", Family::Retro),
    ("msx1", "MSX", Family::Retro),
    ("msx2+", "MSX2+", Family::Retro),
    ("msxturbor", "MSX turbo R", Family::Retro),
    ("mugen", "M.U.G.E.N", Family::Pc),
    ("multivision", "Multivision", Family::Otros),
    ("n64", "Nintendo 64", Family::Nintendo),
    ("n64dd", "Nintendo 64DD", Family::Nintendo),
    ("naomi", "Sega NAOMI", Family::Arcade),
    ("naomi2", "Sega NAOMI 2", Family::Arcade),
    ("naomigd", "Sega NAOMI GD-ROM", Family::Arcade),
    ("nds", "Nintendo DS", Family::Nintendo),
    ("neogeo", "Neo Geo", Family::Arcade),
    ("neogeo64", "Neo Geo 64", Family::Arcade),
    ("neogeocd", "Neo Geo CD", Family::Arcade),
    ("nes", "Nintendo NES", Family::Nintendo),
    ("ngage", "Nokia N-Gage", Family::Otros),
    ("ngp", "Neo Geo Pocket", Family::Otros),
    ("ngpc", "Neo Geo Pocket Color", Family::Otros),
    ("o2em", "Odyssey 2 (O2EM)", Family::Retro),
    ("odyssey2", "Magnavox Odyssey 2", Family::Retro),
    ("openbor", "OpenBOR", Family::Pc),
    ("oricatmos", "Oric Atmos", Family::Retro),
    ("pc88", "NEC PC-88", Family::Retro),
    ("pc98", "NEC PC-98", Family::Retro),
    ("pcengine", "PC Engine", Family::Otros),
    ("pcenginecd", "PC Engine CD", Family::Otros),
    ("peliculas", "Peliculas", Family::Otros),
    ("pet", "Commodore PET", Family::Retro),
    ("pico8", "PICO-8", Family::Otros),
    ("pinballfx3", "Pinball FX3", Family::Pc),
    ("pokemini", "Pokemon Mini", Family::Nintendo),
    ("ps2", "PlayStation 2", Family::Sony),
    ("ps3", "PlayStation 3", Family::Sony),
    ("ps4", "PlayStation 4", Family::Sony),
    ("psp", "PlayStation Portable", Family::Sony),
    ("psvita", "PlayStation Vita", Family::Sony),
    ("psx", "PlayStation", Family::Sony),
    ("pv1000", "Casio PV-1000", Family::Otros),
    ("revistas", "Revistas", Family::Otros),
    ("samcoupe", "SAM Coupe", Family::Retro),
    ("satellaview", "Satellaview", Family::Nintendo),
    ("saturn", "Sega Saturn", Family::Sega),
    ("scummvm", "ScummVM", Family::Pc),
    ("scv", "Super Cassette Vision", Family::Otros),
    ("sega32x", "Sega 32X", Family::Sega),
    ("segacd", "Sega CD", Family::Sega),
    ("segastv", "Sega ST-V", Family::Arcade),
    ("sg1000", "Sega SG-1000", Family::Sega),
    ("snes", "Super Nintendo", Family::Nintendo),
    ("snes-msu1", "Super Nintendo MSU-1", Family::Nintendo),
    ("spectravideo", "Spectravideo", Family::Retro),
    ("sufami", "Sufami Turbo", Family::Nintendo),
    ("supervision", "Watara Supervision", Family::Otros),
    ("supracan", "Super A'Can", Family::Otros),
    ("switch", "Nintendo Switch", Family::Nintendo),
    ("teknoparrot", "TeknoParrot", Family::Arcade),
    ("triforce", "Triforce", Family::Arcade),
    ("uzebox", "Uzebox", Family::Otros),
    ("vc4000", "Interton VC 4000", Family::Otros),
    ("vectrex", "Vectrex", Family::Retro),
    ("vircon32", "Vircon32", Family::Otros),
    ("virtualboy", "Virtual Boy", Family::Nintendo),
    ("vsmile", "V.Smile", Family::Otros),
    ("wasm4", "WASM-4", Family::Otros),
    ("wii", "Nintendo Wii", Family::Nintendo),
    ("wiiu", "Nintendo Wii U", Family::Nintendo),
    ("wiiware", "WiiWare", Family::Nintendo),
    ("windows", "Windows", Family::Pc),
    ("wswan", "WonderSwan", Family::Otros),
    ("wswanc", "WonderSwan Color", Family::Otros),
    ("x1", "Sharp X1", Family::Retro),
    ("x68000", "Sharp X68000", Family::Retro),
    ("xbox", "Xbox", Family::Microsoft),
    ("xbox360", "Xbox 360", Family::Microsoft),
    ("xegs", "Atari XEGS", Family::Retro),
    ("zmachine", "Z-machine", Family::Otros),
    ("zx81", "Sinclair ZX81", Family::Retro),
    ("zxspectrum", "ZX Spectrum", Family::Retro),
];

fn lookup(id: &str) -> Option<&'static (&'static str, &'static str, Family)> {
    TABLE.binary_search_by(|probe| probe.0.cmp(id)).ok().map(|i| &TABLE[i])
}

/// Nombre legible; si el id no esta en la tabla se capitaliza tal cual.
pub fn display_name(id: &str) -> String {
    match lookup(id) {
        Some((_, name, _)) => name.to_string(),
        None => {
            let mut chars = id.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

pub fn family(id: &str) -> Family {
    lookup(id).map(|(_, _, family)| *family).unwrap_or(Family::Otros)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_tabla_esta_ordenada_para_la_busqueda_binaria() {
        for pair in TABLE.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} deberia ir despues de {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn traduce_ids_conocidos_y_desconocidos() {
        assert_eq!(display_name("psx"), "PlayStation");
        assert_eq!(family("psx"), Family::Sony);
        assert_eq!(display_name("plataforma_rara"), "Plataforma_rara");
        assert_eq!(family("plataforma_rara"), Family::Otros);
    }
}
