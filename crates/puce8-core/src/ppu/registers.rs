//! Registres de la PPU vus par le CPU ($2000-$2007) — E13a : $2000, $2001, lecture $2002.
// wiki: PPU_registers ; wiki: PPU_masks_and_control

/// Registre de contrôle $2000 (bit 7 = NMI activée).
pub struct Registers {
    /// $2000 : bit 7 = NMI activée.
    pub ctrl: u8,
    /// $2001 : bits 3-4 = rendu actif.
    pub mask: u8,
    /// $2002-$2007 : mémorisées seulement pour l'instant. // E15
    other: [u8; 6],
    /// Valeur renvoyée par les lectures de $2003-$2007 (toujours 0 pour l'instant).
    pub io_latch: u8,
    /// Effacé à chaque lecture de $2002.
    pub w: bool,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    pub fn new() -> Self {
        Registers {
            ctrl: 0,
            mask: 0,
            other: [0; 6],
            io_latch: 0,
            w: false,
        }
    }

    /// Écriture CPU : $2000 → `ctrl`, $2001 → `mask`, autres mémorisées seulement. // E15
    pub fn write(&mut self, reg: u8, v: u8) {
        match reg {
            0 => self.ctrl = v,
            1 => self.mask = v,
            n @ 2..=7 => self.other[usize::from(n - 2)] = v,
            _ => {}
        }
    }

    /// Lecture CPU : $2002 → (VBlank << 7) | io_latch & 0x1F, puis `w` = false ; autres → io_latch.
    pub fn read(&mut self, reg: u8, vblank: bool) -> u8 {
        match reg {
            2 => {
                let value = (u8::from(vblank) << 7) | (self.io_latch & 0x1F);
                self.w = false; // $2002 efface les drapeaux de statut
                value
            }
            _ => self.io_latch,
        }
    }

    /// Lecture sans effet : même valeur que `read`, sans effacer `w`.
    pub fn peek(&self, reg: u8, vblank: bool) -> u8 {
        match reg {
            2 => (u8::from(vblank) << 7) | (self.io_latch & 0x1F),
            _ => self.io_latch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecritures_2000_2001() {
        let mut r = Registers::new();
        r.write(0, 0x84);
        r.write(1, 0x1C);
        assert_eq!(r.ctrl, 0x84);
        assert_eq!(r.mask, 0x1C);
    }

    #[test]
    fn ecritures_autres_memorisees() {
        let mut r = Registers::new();
        for (reg, v) in [
            (2u8, 0xAA),
            (3, 0xBB),
            (4, 0xCC),
            (5, 0xDD),
            (6, 0xEE),
            (7, 0xFF),
        ] {
            r.write(reg, v);
            assert_eq!(r.other[usize::from(reg - 2)], v);
        }
    }

    #[test]
    fn lecture_2003_renvoye_io_latch() {
        let mut r = Registers::new();
        for reg in [3u8, 4, 5, 6, 7] {
            assert_eq!(r.read(reg, true), 0); // io_latch toujours 0 pour l'instant
        }
    }

    #[test]
    fn lecture_2002_efface_w() {
        let mut r = Registers::new();
        r.w = true;
        assert_eq!(r.read(2, false), 0x00);
        assert!(!r.w);
        assert_eq!(r.peek(2, true), 0x80); // peek sans effet
    }
}
