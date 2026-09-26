//! Registres de la PPU vus par le CPU ($2000-$2007).
//! E13a : $2000, $2001, lecture $2002. E15a : registres internes v, t, x et latch d'open bus.
// wiki: PPU_registers ; wiki: PPU_masks_and_control

/// Registre de controle $2000 (bit 7 = NMI activee) + registres internes v, t, x.
pub struct Registers {
    /// $2000 : bit 7 = NMI activee.
    pub ctrl: u8,
    /// $2001 : bits 3-4 = rendu actif.
    pub mask: u8,
    /// Latch d'open bus : derniere valeur ecrite dans $2000-$2007.
    pub io_latch: u8,
    /// 1re ou 2e ecriture de $2005/$2006 ; efface par une lecture $2002.
    pub w: bool,
    /// v : adresse PPU courante (15 bits).
    pub v: u16,
    /// t : registre temporaire defilement/adresse (15 bits) : yyy NN YYYYY XXXXX.
    pub t: u16,
    /// x : fine X (3 bits).
    pub x: u8,
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
            io_latch: 0,
            w: false,
            v: 0,
            t: 0,
            x: 0,
        }
    }

    /// Ecriture CPU : $2000 -> ctrl + bits 10-11 de t ; $2001 -> mask ;
    /// $2005/$2006 -> defilement/adresse (compteur `w`) ; toutes les ecritures -> io_latch.
    pub fn write(&mut self, reg: u8, v: u8) {
        match reg {
            0 => {
                self.ctrl = v;
                // Bits 10-11 de t : selection du nametable (bit 9 conserve).
                self.t = (self.t & 0xF3FF) | ((u16::from(v & 3)) << 10);
            }
            1 => self.mask = v,
            5 if !self.w => {
                // 1re ecriture $2005 : coarse X (bits 0-4 de t) + fine X.
                self.t = (self.t & 0xFFE0) | u16::from(v >> 3);
                self.x = v & 7;
                self.w = true;
            }
            5 => {
                // 2e ecriture $2005 : fine Y (bits 12-14 de t) + coarse Y (bits 5-8).
                self.t =
                    (self.t & 0x8C1F) | ((u16::from(v & 7)) << 12) | ((u16::from(v & 0xF8)) << 2);
                self.w = false;
            }
            6 if !self.w => {
                // 1re ecriture $2006 : bits 8-13 de t.
                self.t = (self.t & 0x00FF) | ((u16::from(v & 0x3F)) << 8);
                self.w = true;
            }
            6 => {
                // 2e ecriture $2006 : bits 0-7 de t, puis v = t.
                self.t = (self.t & 0xFF00) | u16::from(v);
                self.v = self.t;
                self.w = false;
            }
            _ => {}
        }
        // Toute ecriture dans $2000-$2007 met a jour le latch d'open bus.
        self.io_latch = v;
    }

    /// Lecture CPU : $2002 -> (VBlank << 7) | io_latch & 0x1F, puis `w` = false ; autres -> io_latch.
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

    /// Lecture sans effet : meme valeur que `read`, sans effacer `w`.
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
    fn t_2000() {
        let mut r = Registers::new();
        r.write(0, 0x03); // bits 10-11 de t = 3
        assert_eq!(r.ctrl, 0x03);
        assert_eq!(r.t, 0x0C00);
    }

    #[test]
    fn scroll_2005() {
        let mut r = Registers::new();
        r.write(5, 0x7D); // 1re ecriture : coarse X + fine X
        assert_eq!(r.t, 0x000F);
        assert_eq!(r.x, 5);
        assert!(r.w);
        r.write(5, 0x5E); // 2e ecriture : fine Y + coarse Y
        assert_eq!(r.t, 0x616F);
        assert_eq!(r.x, 5);
        assert!(!r.w);
    }

    #[test]
    fn adresse_2006() {
        let mut r = Registers::new();
        r.write(6, 0x21); // 1re ecriture : bits 8-13 de t
        assert_eq!(r.t, 0x2100);
        assert!(r.w);
        r.write(6, 0x08); // 2e ecriture : v = t
        assert_eq!(r.v, 0x2108);
        assert!(!r.w);
    }

    #[test]
    fn lecture_2002_reset_w() {
        let mut r = Registers::new();
        r.write(6, 0x21); // 1re ecriture $2006 -> w = 1
        assert!(r.w);
        r.read(2, false); // lecture $2002 -> w = 0
        assert!(!r.w);
        r.write(6, 0x34); // redevient une 1re ecriture
        r.write(6, 0x56); // puis 2e ecriture
        assert_eq!(r.v, 0x3456);
    }

    #[test]
    fn open_bus_2002() {
        let mut r = Registers::new();
        r.write(0, 0x1F); // io_latch = 0x1F
        assert_eq!(r.read(2, false), 0x1F); // bits 0-4 = 0x1F
    }

    #[test]
    fn ecritures_toutes_mettent_io_latch() {
        let mut r = Registers::new();
        for reg in [0u8, 1, 2, 3, 4, 5, 6, 7] {
            r.write(reg, 0x42);
            assert_eq!(r.io_latch, 0x42);
        }
    }

    #[test]
    fn lecture_2003_renvoye_io_latch() {
        let mut r = Registers::new();
        for reg in [3u8, 4, 5, 6, 7] {
            assert_eq!(r.read(reg, true), 0); // registre neuf : io_latch a sa valeur initiale
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
