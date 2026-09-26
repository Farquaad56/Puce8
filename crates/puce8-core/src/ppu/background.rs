//! Fond PPU (E18a) : increments et copies du registre v (fonctions pures).
//! Layout de v : yyy NN YYYYY XXXXX (fine Y, nametable, coarse Y, coarse X).

/// Incremente coarse X ; a 31 : repasse a 0 et bascule la nametable horizontale.
pub fn inc_coarse_x(v: u16) -> u16 {
    if (v & 0x001F) == 31 {
        (v & !0x001F) ^ 0x0400
    } else {
        v.wrapping_add(1)
    }
}

/// Incremente fine Y ; au debordement, incremente coarse Y (29 -> 0 + bascule NT verticale ; 31 -> 0 sans bascule).
pub fn inc_y(v: u16) -> u16 {
    if (v & 0x7000) != 0x7000 {
        return v.wrapping_add(0x1000);
    }
    let mut v = v & !0x7000;
    let mut y = (v >> 5) & 31;
    if y == 29 {
        y = 0;
        v ^= 0x0800;
    } else if y == 31 {
        y = 0;
    } else {
        y += 1;
    }
    (v & !0x03E0) | (y << 5)
}

/// Copie les bits horizontaux de t dans v (coarse X + nametable X).
pub fn copy_x(v: u16, t: u16) -> u16 {
    (v & !0x041F) | (t & 0x041F)
}

/// Copie les bits verticaux de t dans v (fine Y + nametable Y + coarse Y).
pub fn copy_y(v: u16, t: u16) -> u16 {
    (v & !0x7BE0) | (t & 0x7BE0)
}

/// Registres a decalage du fond (E18c2) : 16 bits chacun ; le pixel courant est au bit 15 - fine_x.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BgShifters {
    pub pat_lo: u16,
    pub pat_hi: u16,
    pub at_lo: u16,
    pub at_hi: u16,
}

impl BgShifters {
    /// Decale les 4 registres de 1 bit vers la gauche.
    pub fn shift(&mut self) {
        self.pat_lo <<= 1;
        self.pat_hi <<= 1;
        self.at_lo <<= 1;
        self.at_hi <<= 1;
    }

    /// Recharge les 8 bits BAS depuis les latches (les 8 bits hauts sont conserves).
    /// `at` = attribut deja reduit a 2 bits (0-3) : bit 0 -> at_lo, bit 1 -> at_hi (etendus a 8 bits).
    pub fn reload(&mut self, pat_lo: u8, pat_hi: u8, at: u8) {
        self.pat_lo = (self.pat_lo & 0xFF00) | u16::from(pat_lo);
        self.pat_hi = (self.pat_hi & 0xFF00) | u16::from(pat_hi);
        self.at_lo = (self.at_lo & 0xFF00) | if at & 1 != 0 { 0xFF } else { 0x00 };
        self.at_hi = (self.at_hi & 0xFF00) | if at & 2 != 0 { 0xFF } else { 0x00 };
    }

    /// (px, pal) du pixel courant : px = couleur 0-3 du motif, pal = palette 0-3.
    pub fn pixel(&self, fine_x: u8) -> (u8, u8) {
        let bit = 15 - u16::from(fine_x & 7);
        let lo = ((self.pat_lo >> bit) & 1) as u8;
        let hi = ((self.pat_hi >> bit) & 1) as u8;
        let alo = ((self.at_lo >> bit) & 1) as u8;
        let ahi = ((self.at_hi >> bit) & 1) as u8;
        ((hi << 1) | lo, (ahi << 1) | alo)
    }

    /// Un point du pipeline (lignes de rendu, rendu actif). ORDRE : decalage PUIS rechargement.
    /// Decalage aux points 2-257 et 322-337 ; rechargement quand dot % 8 == 1 aux points 9-257 et 329-337.
    pub fn step(&mut self, dot: u16, pat_lo: u8, pat_hi: u8, at: u8) {
        if (2..=257).contains(&dot) || (322..=337).contains(&dot) {
            self.shift();
        }
        if dot % 8 == 1 && ((9..=257).contains(&dot) || (329..=337).contains(&dot)) {
            self.reload(pat_lo, pat_hi, at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inc_x_simple() {
        assert_eq!(inc_coarse_x(0x0000), 0x0001);
        assert_eq!(inc_coarse_x(0x0005), 0x0006);
    }

    #[test]
    fn inc_x_31_change_nt() {
        assert_eq!(inc_coarse_x(0x001F), 0x0400);
        assert_eq!(inc_coarse_x(0x041F), 0x0000);
    }

    #[test]
    fn inc_y_fin() {
        assert_eq!(inc_y(0x1000), 0x2000);
    }

    #[test]
    fn inc_y_29() {
        let v = 0x7000 | (29 << 5); // fine Y = 7, coarse Y = 29
        assert_eq!(inc_y(v), 0x0800);
        assert_eq!(inc_y(v | 0x0800), 0x0000);
    }

    #[test]
    fn inc_y_31() {
        let v = 0x7000 | (31 << 5); // fine Y = 7, coarse Y = 31
        assert_eq!(inc_y(v), 0x0000);
    }

    #[test]
    fn copy_x_bits() {
        assert_eq!(copy_x(0x7BE0, 0x041F), 0x7FFF);
        assert_eq!(copy_x(0x7FFF, 0x0000), 0x7BE0);
    }

    #[test]
    fn copy_y_bits() {
        assert_eq!(copy_y(0x041F, 0x7BE0), 0x7FFF);
        assert_eq!(copy_y(0x7FFF, 0x0000), 0x041F);
    }

    #[test]
    fn reload_bas_seulement() {
        let mut s = BgShifters {
            pat_lo: 0xAB00,
            pat_hi: 0xCD00,
            at_lo: 0xFF00,
            at_hi: 0x0000,
        };
        s.reload(0x12, 0x34, 0b10);
        assert_eq!(s.pat_lo, 0xAB12);
        assert_eq!(s.pat_hi, 0xCD34);
        assert_eq!(s.at_lo, 0xFF00);
        assert_eq!(s.at_hi, 0x00FF);
    }

    #[test]
    fn pixel_fine_x() {
        let s = BgShifters {
            pat_lo: 0x8000,
            pat_hi: 0x4000,
            at_lo: 0xC000,
            at_hi: 0x8000,
        };
        assert_eq!(s.pixel(0), (0b01, 0b11));
        assert_eq!(s.pixel(1), (0b10, 0b01));
        assert_eq!(s.pixel(2), (0, 0));
    }

    /// Simule une ligne : prechargement 321-336 (tuiles 0 et 1), puis points 1-256.
    /// Le motif bas de la tuile k vaut k * 37 + 11 ; pour chaque fine_x, le pixel x doit etre
    /// le bit 7 - ((x + fine_x) % 8) de la tuile (x + fine_x) / 8.
    #[test]
    fn ligne_complete_ordre_decalage_puis_rechargement() {
        let tuile = |k: usize| ((k * 37 + 11) & 0xFF) as u8;
        for fine_x in 0..8u8 {
            let mut s = BgShifters::default();
            let mut latch = 0u8;
            let mut k = 0usize;
            let points = (321..=340u16).chain(1..=256u16);
            for (n, dot) in points.enumerate() {
                s.step(dot, latch, 0, 0);
                if n >= 20 {
                    let x = usize::from(dot - 1);
                    let (px, _) = s.pixel(fine_x);
                    let pos = x + usize::from(fine_x);
                    let attendu = (tuile(pos / 8) >> (7 - pos % 8)) & 1;
                    assert_eq!(px, attendu, "fine_x {fine_x} x {x}");
                }
                // Le motif (haut) d'une tuile est pret en phase 7 (fetchs E18b).
                if ((1..=256).contains(&dot) || (321..=336).contains(&dot)) && dot % 8 == 7 {
                    latch = tuile(k);
                    k += 1;
                }
            }
        }
    }
}
