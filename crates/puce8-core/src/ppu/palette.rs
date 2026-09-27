// FICHIER GENERE - palette NTSC 2C02 par defaut (annexe B). Index $00-$3F -> 0x00RRGGBB.
pub const NES_PALETTE: [u32; 64] = [
    0x545454, 0x001E74, 0x081090, 0x300088, 0x440064, 0x5C0030, 0x540400, 0x3C1800, 0x202A00,
    0x083A00, 0x004000, 0x003C00, 0x00323C, 0x000000, 0x000000, 0x000000, 0x989698, 0x084CC4,
    0x3032EC, 0x5C1EE4, 0x8814B0, 0xA01464, 0x982220, 0x783C00, 0x545A00, 0x287200, 0x087C00,
    0x007628, 0x006678, 0x000000, 0x000000, 0x000000, 0xECEEEC, 0x4C9AEC, 0x787CEC, 0xB062EC,
    0xE454EC, 0xEC58B4, 0xEC6A64, 0xD48820, 0xA0AA00, 0x74C400, 0x4CD020, 0x38CC6C, 0x38B4CC,
    0x3C3C3C, 0x000000, 0x000000, 0xECEEEC, 0xA8CCEC, 0xBCBCEC, 0xD4B2EC, 0xECAEEC, 0xECAED4,
    0xECB4B0, 0xE4C490, 0xCCD278, 0xB4DE78, 0xA8E290, 0x98E2B4, 0xA0D6E4, 0xA0A2A0, 0x000000,
    0x000000,
];

/// Couleur 0x00RRGGBB d'un pixel du framebuffer : bits 0-5 = index palette,
/// bits 6-8 = emphase (bit 6 = R, 7 = G, 8 = B). Les composantes NON accentuees
/// sont multipliees par 3/4 (division entiere).
pub fn to_rgba(px: u16) -> u32 {
    let base = NES_PALETTE[usize::from(px & 0x3F)];
    let emphase = (px >> 6) & 7;
    if emphase == 0 {
        return base;
    }
    let mut r = (base >> 16) & 0xFF;
    let mut g = (base >> 8) & 0xFF;
    let mut b = base & 0xFF;
    if emphase & 1 == 0 {
        r = r * 3 / 4;
    }
    if emphase & 2 == 0 {
        g = g * 3 / 4;
    }
    if emphase & 4 == 0 {
        b = b * 3 / 4;
    }
    (r << 16) | (g << 8) | b
}

/// Lecture $2007 en palette ($3Fxx) : valeur palette | open bus (bits 6-7) ;
/// en mode niveaux de gris, le resultat est masque a $30.
pub fn read_2007(value: u8, io_latch: u8, gris: bool) -> u8 {
    let v = value | (io_latch & 0xC0);
    if gris {
        v & 0x30
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_rgba_blanc() {
        assert_eq!(to_rgba(0x30), 0xECEEEC);
    }

    #[test]
    fn to_rgba_emphase_rouge() {
        assert_eq!(to_rgba(0x30 | (1 << 6)), 0xECB2B1);
    }

    #[test]
    fn emphase_rouge() {
        let c = to_rgba(0x30 | (1 << 6)); // blanc + emphase rouge
        assert_eq!((c >> 16) & 0xFF, 0xEC); // R intact
        assert_eq!((c >> 8) & 0xFF, 0xEE * 3 / 4); // G attenuee par 3/4
        assert_eq!(c & 0xFF, 0xEC * 3 / 4); // B attenuee par 3/4
    }

    #[test]
    fn read_2007_couleur_et_gris() {
        assert_eq!(read_2007(0x3A, 0x7A, false), 0x7A); // 0x3A | (0x7A & $C0)
        assert_eq!(read_2007(0x3A, 0x7A, true), 0x30); // masque a $30 en gris
    }

    #[test]
    fn to_rgba_emphase_totale() {
        assert_eq!(to_rgba(0x30 | (7 << 6)), 0xECEEEC);
    }
}
