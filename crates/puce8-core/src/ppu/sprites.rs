//! Evaluation des sprites (E20a1) : OAM secondaire, 8 sprites maximum, au point pres.
//! Machine a etats PURE (aucun acces au Ppu) : `step` est appele a chaque point 1-256
//! d'une ligne visible avec rendu actif ; elle prepare la ligne SUIVANTE.

use super::Ppu;

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
    /// Overflow detecte sur cette ligne (E20b1) ; recopie dans `Ppu::sprite_overflow` (E20b2).
    pub overflow: bool,
    /// Octets restant a lire apres la detection d'overflow (E20b1).
    skip: u8,
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
            overflow: false,
            skip: 0,
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
            self.overflow_step(line, height); // E20b1
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

    /// Recherche d'overflow, une fois 8 sprites trouves (E20b1), AVEC le bogue materiel :
    /// l'octet lu OAM[n][m] est compare comme un Y ;
    /// - dans la plage : overflow = 1, puis lecture des 3 octets suivants (m++ avec report sur n), puis fin ;
    /// - hors plage : n += 1 ET m += 1 SANS report (bogue "diagonal") ; apres le 64e sprite : fin.
    fn overflow_step(&mut self, line: u16, height: u16) {
        if self.skip > 0 {
            self.skip -= 1;
            self.m += 1;
            if self.m == 4 {
                self.m = 0;
                self.next_sprite();
            }
            if self.skip == 0 {
                self.done = true;
            }
            return;
        }
        if Self::in_range(line, self.latch, height) {
            self.overflow = true;
            self.skip = 3;
            self.m += 1;
            if self.m == 4 {
                self.m = 0;
                self.next_sprite();
            }
        } else {
            self.m = (self.m + 1) & 3; // bogue : m avance aussi, sans report sur n
            self.next_sprite();
        }
    }

    /// Octet sur le bus interne de l'evaluation (dernier octet lu dans l'OAM) : lu par `$2004` (E20b3).
    pub fn bus_value(&self) -> u8 {
        self.latch
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

impl Ppu {
    /// Hauteur des sprites : 16 si `ctrl & 0x20`, sinon 8.
    pub fn sprite_height(&self) -> u16 {
        if self.regs.ctrl & 0x20 != 0 {
            16
        } else {
            8
        }
    }

    /// Travail des sprites du point courant, appele par `tick` apres `bg_fetch` (E20a2).
    /// SIMPLIFICATION: l'evaluation commence toujours au sprite 0 (oam_addr ignore), revu en E36.
    pub(super) fn sprite_eval_dot(&mut self) {
        if self.regs.mask & 0x18 == 0 {
            return; // rendu inactif : ni evaluation, ni remise a zero de oam_addr
        }
        let (line, dot) = (self.line, self.point);
        if line <= 239 && (1..=256).contains(&dot) {
            let height = self.sprite_height();
            self.sprite_eval.step(dot, &self.oam, line, height);
            if self.sprite_eval.overflow {
                self.sprite_overflow = true; // E20b2 : reste pose jusqu'en (261, 1)
            }
        }
        if (line <= 239 || line == 261) && (257..=320).contains(&dot) {
            self.regs.oam_addr = 0;
        }
    }

    /// Bits "sprites" de `$2002` (E20b2/E21c1) : bit 5 = overflow, bit 6 = sprite 0 hit.
    /// `r` = numero du registre (0-7) ; renvoie 0 pour les autres registres.
    pub(super) fn status_sprites(&self, r: usize) -> u8 {
        if r != 2 {
            return 0;
        }
        (u8::from(self.sprite0_hit) << 6) | (u8::from(self.sprite_overflow) << 5)
        // E21c1 : bit 6
    }

    /// Vrai si l'OAM est occupee par le rendu (E20b3) : rendu actif, lignes 0-239 et 261.
    /// Une ecriture `$2004` n'ecrit alors rien et fait seulement `oam_addr += 4`.
    pub(super) fn oam_busy(&self) -> bool {
        self.regs.mask & 0x18 != 0 && (self.line <= 239 || self.line == 261)
    }

    /// Lecture `$2004` pendant l'evaluation (lignes 0-239, points 65-256, rendu actif) :
    /// renvoie l'octet que l'evaluation vient de lire dans l'OAM (E20b3).
    /// SIMPLIFICATION: points 257-320 (fetchs des sprites) non geres ici, revu en E21/E36.
    pub(super) fn oam_eval_read(&self) -> Option<u8> {
        if self.regs.mask & 0x18 != 0 && self.line <= 239 && (65..=256).contains(&self.point) {
            Some(self.sprite_eval.bus_value())
        } else {
            None
        }
    }

    /// Vrai pendant l'effacement de l'OAM secondaire (lignes 0-239, points 1-64, rendu actif) :
    /// une lecture de `$2004` renvoie alors `$FF`.
    pub(super) fn oam_clear_read(&self) -> bool {
        self.regs.mask & 0x18 != 0 && self.line <= 239 && (1..=64).contains(&self.point)
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

    // ---------- E20a2 : branchement dans le Ppu ----------

    use crate::bus::Bus;

    /// PPU en (line, 0) avec rendu actif et une OAM hors ecran.
    fn ppu_ligne(line: u16) -> (Ppu, Bus) {
        let mut ppu = Ppu::new();
        ppu.oam = oam_vide();
        ppu.regs.mask = 0x18;
        ppu.line = line;
        ppu.point = 0;
        (ppu, Bus::for_test_with_prg(&[0xA9]))
    }

    fn jusqu_a(ppu: &mut Ppu, bus: &mut Bus, cible: (u16, u16)) {
        while ppu.position() != cible {
            ppu.tick(&mut (*bus.mapper));
        }
    }

    #[test]
    fn eval_dans_tick() {
        let (mut ppu, mut bus) = ppu_ligne(10);
        ppu.oam[12] = 9; // sprite 3 : Y = 9, ligne 10 dans la plage
        jusqu_a(&mut ppu, &mut bus, (10, 256));
        assert_eq!(ppu.sprite_eval.found, 1);
        assert_eq!(&ppu.sprite_eval.secondary[0..4], &ppu.oam[12..16]);
    }

    #[test]
    fn oam_addr_257() {
        let (mut ppu, mut bus) = ppu_ligne(10);
        ppu.regs.oam_addr = 0x55;
        jusqu_a(&mut ppu, &mut bus, (10, 256));
        assert_eq!(ppu.regs.oam_addr, 0x55);
        jusqu_a(&mut ppu, &mut bus, (10, 257));
        assert_eq!(ppu.regs.oam_addr, 0);
    }

    #[test]
    fn lecture_2004_effacement() {
        let (mut ppu, mut bus) = ppu_ligne(10);
        ppu.oam[0] = 0x42;
        jusqu_a(&mut ppu, &mut bus, (10, 30));
        assert_eq!(ppu.cpu_read_register(4, &mut (*bus.mapper)), 0xFF);
        ppu.regs.mask = 0; // rendu coupe : lecture normale
        assert_eq!(ppu.cpu_read_register(4, &mut (*bus.mapper)), 0x42);
    }

    #[test]
    fn pas_d_eval_sans_rendu() {
        let (mut ppu, mut bus) = ppu_ligne(10);
        ppu.regs.mask = 0;
        ppu.oam[12] = 9;
        ppu.regs.oam_addr = 0x55;
        jusqu_a(&mut ppu, &mut bus, (10, 300));
        assert_eq!(ppu.sprite_eval.found, 0);
        assert_eq!(ppu.regs.oam_addr, 0x55);
    }

    // ---------- E20b1 : overflow (avec le bogue) ----------
    // Ligne 200 : les octets 1-3 de `oam_vide` (0-63) ne sont jamais "dans la plage" quand le bogue les lit comme des Y.

    #[test]
    fn huit_sans_overflow() {
        let mut oam = oam_vide();
        for n in 0..8 {
            oam[n * 4] = 200;
        }
        let e = ligne(&oam, 200, 8);
        assert_eq!(e.found, 8);
        assert!(!e.overflow);
    }

    #[test]
    fn overflow_9() {
        let mut oam = oam_vide();
        for n in 0..9 {
            oam[n * 4] = 200;
        }
        let e = ligne(&oam, 200, 8);
        assert!(e.overflow);
        assert_eq!(e.found, 8); // l'OAM secondaire ne contient que 8 sprites
    }

    #[test]
    fn overflow_bogue_diagonal() {
        // 8 dans la plage ; sprite 8 hors plage -> n = 9, m = 1 ; OAM[9][1] lu comme un Y.
        let mut oam = oam_vide();
        for n in 0..8 {
            oam[n * 4] = 200;
        }
        oam[9 * 4 + 1] = 198; // "Y" = 198 : ligne 200 dans la plage
        assert!(ligne(&oam, 200, 8).overflow);
    }

    #[test]
    fn overflow_bogue_faux_negatif() {
        // 8 dans la plage ; sprite 9 VRAIMENT dans la plage, mais son Y n'est jamais lu
        // (on lit OAM[9][1], puis OAM[10][2]...) : pas d'overflow, comme sur le vrai materiel.
        let mut oam = oam_vide();
        for n in 0..8 {
            oam[n * 4] = 200;
        }
        oam[9 * 4] = 200;
        assert!(!ligne(&oam, 200, 8).overflow);
    }

    // ---------- E20b2 : drapeau d'overflow dans le Ppu ($2002 bit 5) ----------

    /// PPU en (200, 0), rendu actif, 9 sprites sur la ligne 200.
    fn ppu_9_sprites() -> (Ppu, Bus) {
        let (mut ppu, bus) = ppu_ligne(200);
        for n in 0..9 {
            ppu.oam[n * 4] = 200;
        }
        (ppu, bus)
    }

    #[test]
    fn overflow_2002() {
        let (mut ppu, mut bus) = ppu_9_sprites();
        assert_eq!(ppu.cpu_peek_register(2) & 0x20, 0);
        jusqu_a(&mut ppu, &mut bus, (200, 257));
        assert!(ppu.sprite_overflow);
        assert_eq!(ppu.cpu_peek_register(2) & 0x20, 0x20);
        // La lecture de $2002 n'efface PAS l'overflow (seul VBlank est efface).
        assert_eq!(ppu.cpu_read_register(2, &mut (*bus.mapper)) & 0x20, 0x20);
        assert_eq!(ppu.cpu_read_register(2, &mut (*bus.mapper)) & 0x20, 0x20);
    }

    #[test]
    fn overflow_efface_261() {
        let (mut ppu, mut bus) = ppu_9_sprites();
        jusqu_a(&mut ppu, &mut bus, (200, 257));
        ppu.oam = oam_vide(); // plus de sprites : l'overflow reste pose jusqu'a (261, 1)
        jusqu_a(&mut ppu, &mut bus, (261, 0));
        assert!(ppu.sprite_overflow);
        jusqu_a(&mut ppu, &mut bus, (261, 1));
        assert!(!ppu.sprite_overflow);
    }

    // ---------- E20b3 : $2004 pendant le rendu ----------

    #[test]
    fn ecriture_2004_rendu() {
        let (mut ppu, mut bus) = ppu_ligne(10);
        jusqu_a(&mut ppu, &mut bus, (10, 100));
        ppu.regs.oam_addr = 0x10;
        let avant = ppu.oam;
        ppu.cpu_write_register(4, 0xAB, &mut (*bus.mapper));
        assert_eq!(ppu.regs.oam_addr, 0x14); // + 4
        assert_eq!(ppu.oam, avant); // aucune ecriture
        ppu.regs.mask = 0; // rendu coupe : ecriture normale
        ppu.cpu_write_register(4, 0xAB, &mut (*bus.mapper));
        assert_eq!(ppu.oam[0x14], 0xAB);
        assert_eq!(ppu.regs.oam_addr, 0x15);
    }

    #[test]
    fn ecriture_2004_vblank_normale() {
        let (mut ppu, mut bus) = ppu_ligne(245); // VBlank : l'OAM est libre
        ppu.regs.oam_addr = 0x20;
        ppu.cpu_write_register(4, 0x77, &mut (*bus.mapper));
        assert_eq!(ppu.oam[0x20], 0x77);
    }

    #[test]
    fn lecture_2004_evaluation() {
        let (mut ppu, mut bus) = ppu_ligne(10);
        ppu.oam[0] = 0x33; // Y du sprite 0, lu au point 65
        jusqu_a(&mut ppu, &mut bus, (10, 65));
        assert_eq!(ppu.cpu_read_register(4, &mut (*bus.mapper)), 0x33);
    }
}
