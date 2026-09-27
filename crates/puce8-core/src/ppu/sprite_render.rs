//! Rendu des sprites (E21) : adresse des motifs (E21a1), fetchs 257-320 (E21a2),
//! composition avec le fond (E21b), sprite 0 hit (E21c1).

/// Adresse du plan BAS du motif d'un sprite (le plan haut est a +8).
/// `row` = ligne dans le sprite (0 a height - 1) AVANT flip vertical ; `attr & 0x80` = flip vertical.
/// - 8x8 : table choisie par `ctrl & 0x08` ;
/// - 8x16 : table = bit 0 de la tuile, tuile paire = moitie haute, tuile + 1 = moitie basse.
pub fn sprite_pattern_addr(tile: u8, row: u16, attr: u8, height: u16, ctrl: u8) -> u16 {
    let mut row = row;
    if attr & 0x80 != 0 {
        row = height - 1 - row; // flip vertical
    }
    if height == 16 {
        let table = u16::from(tile & 1) * 0x1000;
        let mut t = u16::from(tile & 0xFE);
        if row >= 8 {
            t += 1;
            row -= 8;
        }
        table + t * 16 + row
    } else {
        let table = if ctrl & 0x08 != 0 { 0x1000 } else { 0x0000 };
        table + u16::from(tile) * 16 + row
    }
}

/// Octet de motif charge dans un emplacement : inverse si flip horizontal (`attr & 0x40`).
pub fn load_pattern(byte: u8, attr: u8) -> u8 {
    if attr & 0x40 != 0 {
        byte.reverse_bits()
    } else {
        byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_8x8() {
        assert_eq!(sprite_pattern_addr(0x42, 3, 0x00, 8, 0x08), 0x1423);
        assert_eq!(sprite_pattern_addr(0x42, 3, 0x00, 8, 0x00), 0x0423);
    }

    #[test]
    fn adresse_8x8_flip_v() {
        assert_eq!(sprite_pattern_addr(0x42, 0, 0x80, 8, 0x00), 0x0427);
    }

    #[test]
    fn adresse_8x16_bas() {
        assert_eq!(sprite_pattern_addr(0x43, 10, 0x00, 16, 0x00), 0x1432);
        assert_eq!(sprite_pattern_addr(0x43, 2, 0x00, 16, 0x00), 0x1422);
    }

    #[test]
    fn adresse_8x16_flip() {
        assert_eq!(sprite_pattern_addr(0x42, 0, 0x80, 16, 0x08), 0x0437);
    }

    #[test]
    fn flip_horizontal() {
        assert_eq!(load_pattern(0b1100_0001, 0x40), 0b1000_0011);
        assert_eq!(load_pattern(0b1100_0001, 0x00), 0b1100_0001);
    }
}
