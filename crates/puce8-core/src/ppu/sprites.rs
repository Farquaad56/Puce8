//! Evaluation des sprites (E20a1) : OAM secondaire, 8 sprites maximum, au point pres.
//! Machine a etats PURE (aucun acces au Ppu) : `step` est appele a chaque point 1-256
//! d'une ligne visible avec rendu actif ; elle prepare la ligne SUIVANTE.

/// Etat de l'evaluation des sprites pour une ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteEval {
    /// OAM secondaire : 8 sprites x 4 octets.
    pub secondary: [u8; 32],
    /// Nombre de sprites copies (0-8).
    pub found: u8,
    /// Vrai si le sprite 0 fait partie des sprites copies (pour sprite 0 hit, E21).
    pub sprite0_in_range: bool,
    /// Index du sprite lu dans l'OAM primaire (0-63).
    n: u8,
    /// Octet du sprite (0-3).
    m: u8,
    /// Prochain emplacement libre dans l'OAM secondaire (0-32).
    sec_idx: u8,
    /// Octet lu au point impair, ecrit au point pair suivant.
    latch: u8,
    /// Les 64 sprites ont ete parcourus (ou 8 trouves, voir E20b).
    done: bool,
}

impl Default for SpriteEval {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteEval {
    pub fn new() -> Self {
        SpriteEval {
            secondary: [0xFF; 32],
            found: 0,
            sprite0_in_range: false,
            n: 0,
            m: 0,
            sec_idx: 0,
            latch: 0,
            done: false,
        }
    }

    /// Vrai si le sprite de coordonnee `y` couvre la ligne `line` (hauteur 8 ou 16).
    pub fn in_range(line: u16, y: u8, height: u16) -> bool {
        let diff = i32::from(line) - i32::from(y);
        diff >= 0 && diff < i32::from(height)
    }

    /// Un point (1-256) de l'evaluation sur la ligne `line`.
    /// - 1-64 : effacement de l'OAM secondaire (un octet a $FF tous les 2 points) ;
    /// - 65-256 : point impair = lecture OAM[n][m], point pair = ecriture dans l'OAM secondaire.
    pub fn step(&mut self, dot: u16, oam: &[u8; 256], line: u16, height: u16) {
        match dot {
            1 => {
                *self = SpriteEval::new(); // nouvelle ligne : etat remis a zero
            }
            2..=64 if dot.is_multiple_of(2) => {
                self.secondary[usize::from(dot / 2 - 1)] = 0xFF;
            }
            65..=256 if dot % 2 == 1 => {
                self.latch = oam[usize::from(self.n) * 4 + usize::from(self.m)];
            }
            65..=256 => self.write_step(line, height),
            _ => {}
        }
    }

    /// Point pair (65-256) : copie de l'octet lu, puis avance de n / m.
    fn write_step(&mut self, line: u16, height: u16) {
        if self.done {
            return;
        }
        if self.found >= 8 {
            // 8 sprites deja trouves : la recherche d'overflow viendra en E20b.
            self.done = true;
            return;
        }
        self.secondary[usize::from(self.sec_idx)] = self.latch;
        if self.m == 0 {
            if Self::in_range(line, self.latch, height) {
                if self.n == 0 {
                    self.sprite0_in_range = true;
                }
                self.sec_idx += 1;
                self.m = 1;
            } else {
                self.next_sprite();
            }
        } else {
            self.sec_idx += 1;
            self.m += 1;
            if self.m == 4 {
                self.m = 0;
                self.found += 1;
                self.next_sprite();
            }
        }
    }

    /// Sprite suivant ; apres le 64e, l'evaluation est terminee.
    fn next_sprite(&mut self) {
        self.n += 1;
        if self.n == 64 {
            self.n = 0;
            self.done = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OAM dont tous les Y valent $FF (hors ecran), octets 1-3 = numero du sprite.
    fn oam_vide() -> [u8; 256] {
        let mut oam = [0u8; 256];
        for n in 0..64 {
            oam[n * 4] = 0xFF;
            oam[n * 4 + 1] = n as u8;
            oam[n * 4 + 2] = n as u8;
            oam[n * 4 + 3] = n as u8;
        }
        oam
    }

    /// Joue les points 1-256 d'une ligne.
    fn ligne(oam: &[u8; 256], line: u16, height: u16) -> SpriteEval {
        let mut e = SpriteEval::new();
        e.secondary = [0x00; 32]; // pour verifier l'effacement
        for dot in 1..=256 {
            e.step(dot, oam, line, height);
        }
        e
    }

    #[test]
    fn aucun_sprite() {
        let e = ligne(&oam_vide(), 50, 8);
        assert_eq!(e.found, 0);
        assert_eq!(e.secondary, [0xFF; 32]);
        assert!(!e.sprite0_in_range);
    }

    #[test]
    fn trois_sprites() {
        let mut oam = oam_vide();
        for n in [5usize, 9, 20] {
            oam[n * 4] = 45; // ligne 50 : 50 - 45 = 5 < 8
        }
        let e = ligne(&oam, 50, 8);
        assert_eq!(e.found, 3);
        assert_eq!(&e.secondary[0..4], &[45, 5, 5, 5]);
        assert_eq!(&e.secondary[4..8], &[45, 9, 9, 9]);
        assert_eq!(&e.secondary[8..12], &[45, 20, 20, 20]);
        assert_eq!(e.secondary[12], 0xFF);
    }

    #[test]
    fn huit_max() {
        let mut oam = oam_vide();
        for n in 0..10 {
            oam[n * 4] = 50;
        }
        let e = ligne(&oam, 50, 8);
        assert_eq!(e.found, 8);
        for k in 0..8 {
            assert_eq!(e.secondary[k * 4 + 1], k as u8, "emplacement {k}");
        }
        assert!(e.sprite0_in_range);
    }

    #[test]
    fn sprite_16() {
        let mut oam = oam_vide();
        oam[7 * 4] = 38; // ligne 50 : 50 - 38 = 12
        assert_eq!(ligne(&oam, 50, 8).found, 0); // 8x8 : hors plage
        let e = ligne(&oam, 50, 16);
        assert_eq!(e.found, 1); // 8x16 : dans la plage
        assert_eq!(e.secondary[1], 7);
    }

    #[test]
    fn y_sur_la_ligne() {
        let mut oam = oam_vide();
        oam[0] = 50; // diff = 0 : dans la plage
        oam[4] = 51; // diff = -1 : hors plage
        oam[8] = 42; // diff = 8 : hors plage en 8x8
        let e = ligne(&oam, 50, 8);
        assert_eq!(e.found, 1);
        assert!(e.sprite0_in_range);
    }
}
