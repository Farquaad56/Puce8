//! Rendu des sprites (E21) : adresse des motifs (E21a1), fetchs 257-320 (E21a2),
//! composition avec le fond (E21b), sprite 0 hit (E21c1).

use super::Ppu;
use crate::mapper::Mapper;

/// Un emplacement de sprite charge pour la ligne (motifs deja retournes si flip horizontal).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpriteSlot {
    pub pat_lo: u8,
    pub pat_hi: u8,
    pub attr: u8,
    pub x: u8,
    pub is_sprite0: bool,
}

/// Les 8 emplacements d'une ligne ; seuls les `count` premiers sont valides.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpriteLine {
    pub slots: [SpriteSlot; 8],
    pub count: u8,
}

/// Resultat de la composition des sprites a un x donne (E21b1).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpritePixel {
    /// Couleur 0-3 du sprite gagnant (0 = aucun sprite opaque).
    pub px: u8,
    /// Palette de sprite 0-3 (attr & 3) du gagnant.
    pub pal: u8,
    /// Gagnant derriere le fond (attr & 0x20).
    pub behind: bool,
    /// Le sprite 0 a un pixel opaque a ce x, MEME s'il ne gagne pas (pour le sprite 0 hit, E21c1).
    pub sprite0: bool,
}

impl SpriteLine {
    /// Composition au x donne : le PREMIER emplacement opaque (ordre 0 a 7) l'emporte.
    pub fn pixel(&self, x: u8) -> SpritePixel {
        let mut out = SpritePixel::default();
        for slot in &self.slots[..usize::from(self.count)] {
            let dx = i16::from(x) - i16::from(slot.x);
            if !(0..8).contains(&dx) {
                continue;
            }
            let bit = 7 - dx as u8;
            let px = (((slot.pat_hi >> bit) & 1) << 1) | ((slot.pat_lo >> bit) & 1);
            if px == 0 {
                continue; // transparent : l'emplacement suivant peut gagner
            }
            if slot.is_sprite0 {
                out.sprite0 = true;
            }
            if out.px == 0 {
                out.px = px;
                out.pal = slot.attr & 3;
                out.behind = slot.attr & 0x20 != 0;
            }
        }
        out
    }
}

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

impl Ppu {
    /// Fetchs des sprites (E21a2), appele par `tick` apres `sprite_eval_dot`.
    /// Lignes 0-239 et 261, rendu actif, points 257-320 : 8 emplacements x 8 points.
    /// Point k (1-8) de l'emplacement i : 1 et 3 = lectures NT "poubelle", 5 = motif bas, 7 = motif haut.
    /// Emplacement vide : tuile $FF quand meme lue (donnees ignorees) ; ces acces comptent pour le MMC3.
    /// Ligne 261 : aucun sprite pour la ligne 0 (count = 0).
    pub(super) fn sprite_fetch_dot(&mut self, mapper: &mut dyn Mapper) {
        let (line, dot) = (self.line, self.point);
        if self.regs.mask & 0x18 == 0
            || !(line <= 239 || line == 261)
            || !(257..=320).contains(&dot)
        {
            return;
        }
        let i = usize::from((dot - 257) / 8);
        let found = if line == 261 {
            0
        } else {
            self.sprite_eval.found
        };
        let (y, tile, attr, x) = if i < usize::from(found) {
            let s = &self.sprite_eval.secondary[i * 4..i * 4 + 4];
            (s[0], s[1], s[2], s[3])
        } else {
            (0xFF, 0xFF, 0xFF, 0xFF)
        };
        let height = self.sprite_height();
        let row = line.wrapping_sub(u16::from(y)) & (height - 1);
        let addr = sprite_pattern_addr(tile, row, attr, height, self.regs.ctrl);
        match (dot - 257) % 8 {
            0 | 2 => {
                if dot == 257 {
                    self.sprite_line.count = found;
                }
                let v = self.regs.v;
                let _ = self.read_bg(0x2000 | (v & 0x0FFF), mapper); // NT "poubelle"
            }
            4 => {
                let b = self.read_bg(addr, mapper);
                self.sprite_line.slots[i].pat_lo = load_pattern(b, attr);
            }
            6 => {
                let b = self.read_bg(addr + 8, mapper);
                let slot = &mut self.sprite_line.slots[i];
                slot.pat_hi = load_pattern(b, attr);
                slot.attr = attr;
                slot.x = x;
                slot.is_sprite0 = i == 0 && line != 261 && self.sprite_eval.sprite0_in_range;
            }
            _ => {}
        }
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

    // ---------- E21a2 : fetchs 257-320 dans le Ppu ----------

    use crate::mapper::Mirroring;

    /// Mapper espion : CHR en RAM + journal (point, adresse) de chaque acces notifie.
    struct Espion {
        mem: [u8; 0x2000],
        log: Vec<u16>,
    }

    impl Mapper for Espion {
        fn cpu_read(&mut self, _addr: u16) -> Option<u8> {
            None
        }
        fn cpu_peek(&self, _addr: u16) -> Option<u8> {
            None
        }
        fn cpu_write(&mut self, _addr: u16, _value: u8) {}
        fn ppu_read(&mut self, addr: u16) -> u8 {
            self.mem[usize::from(addr & 0x1FFF)]
        }
        fn ppu_write(&mut self, addr: u16, value: u8) {
            self.mem[usize::from(addr & 0x1FFF)] = value;
        }
        fn ppu_peek(&self, addr: u16) -> u8 {
            self.mem[usize::from(addr & 0x1FFF)]
        }
        fn mirroring(&self) -> Mirroring {
            Mirroring::Horizontal
        }
        fn notify_ppu_address(&mut self, addr: u16) {
            self.log.push(addr);
        }
    }

    fn espion() -> Espion {
        Espion {
            mem: [0; 0x2000],
            log: Vec::new(),
        }
    }

    /// PPU en (line, 0), rendu actif, OAM hors ecran (Y = $FF).
    fn ppu_ligne(line: u16) -> Ppu {
        let mut ppu = Ppu::new();
        ppu.oam = [0xFF; 256];
        ppu.regs.mask = 0x18;
        ppu.line = line;
        ppu.point = 0;
        ppu
    }

    fn jusqu_a(ppu: &mut Ppu, m: &mut Espion, cible: (u16, u16)) {
        while ppu.position() != cible {
            ppu.tick(m);
        }
    }

    #[test]
    fn fetch_un_sprite() {
        let mut ppu = ppu_ligne(20);
        ppu.oam[0..4].copy_from_slice(&[17, 0x42, 0x41, 100]); // Y = 17 : row = 3, palette 1, flip H
        let mut m = espion();
        m.mem[0x0423] = 0b1100_0000; // plan bas, ligne 3 de la tuile $42
        m.mem[0x042B] = 0b0000_0001; // plan haut
        jusqu_a(&mut ppu, &mut m, (20, 320));
        let l = ppu.sprite_line;
        assert_eq!(l.count, 1);
        assert_eq!(l.slots[0].pat_lo, 0b0000_0011); // retourne (flip H)
        assert_eq!(l.slots[0].pat_hi, 0b1000_0000);
        assert_eq!((l.slots[0].attr, l.slots[0].x), (0x41, 100));
        assert!(l.slots[0].is_sprite0);
    }

    #[test]
    fn huit_emplacements_meme_vides() {
        let mut ppu = ppu_ligne(20);
        let mut m = espion();
        jusqu_a(&mut ppu, &mut m, (20, 256));
        m.log.clear();
        jusqu_a(&mut ppu, &mut m, (20, 320));
        // 8 emplacements x (2 NT poubelle + 2 motifs) = 32 acces, meme sans sprite.
        assert_eq!(m.log.len(), 32);
        assert_eq!(ppu.sprite_line.count, 0);
        // Emplacement vide : tuile $FF (table $0000 en 8x8) -> $0FFx.
        assert_eq!(m.log[2] & 0xFFF0, 0x0FF0);
    }

    #[test]
    fn fronts_a12_sprites() {
        // Fond en $0000, sprites en $1000 : aucun front de A12 pendant le fond ; pendant les sprites,
        // un front par emplacement (la lecture NT "poubelle" repasse A12 a 0), le 1er au point 261.
        // Le MMC3 (E28) filtre ces fronts rapproches : il n'en compte qu'UN par ligne.
        let mut ppu = ppu_ligne(20);
        ppu.regs.ctrl = 0x08;
        let mut m = espion();
        jusqu_a(&mut ppu, &mut m, (20, 0));
        let mut fronts = Vec::new();
        let mut a12 = false;
        for _ in 0..341 {
            let avant = m.log.len();
            ppu.tick(&mut m);
            for &a in &m.log[avant..] {
                let haut = a & 0x1000 != 0;
                if haut && !a12 {
                    fronts.push(ppu.point);
                }
                a12 = haut;
            }
        }
        assert_eq!(fronts, vec![261, 269, 277, 285, 293, 301, 309, 317]);
    }

    #[test]
    fn ligne_261_aucun_sprite_pour_la_ligne_0() {
        let mut ppu = ppu_ligne(261);
        ppu.sprite_line.count = 5;
        let mut m = espion();
        jusqu_a(&mut ppu, &mut m, (261, 320));
        assert_eq!(ppu.sprite_line.count, 0);
    }

    // ---------- E21b1 : composition pure (SpriteLine::pixel) ----------

    /// Emplacement plein (couleur 1 sur 8 pixels) a l'abscisse x.
    fn plein(x: u8, attr: u8, sprite0: bool) -> SpriteSlot {
        SpriteSlot {
            pat_lo: 0xFF,
            pat_hi: 0x00,
            attr,
            x,
            is_sprite0: sprite0,
        }
    }

    #[test]
    fn pixel_dans_les_8_colonnes() {
        let mut l = SpriteLine::default();
        l.slots[0] = plein(100, 0x02, false);
        l.count = 1;
        assert_eq!(l.pixel(99).px, 0);
        assert_eq!(l.pixel(100).px, 1);
        assert_eq!(l.pixel(107).px, 1);
        assert_eq!(l.pixel(108).px, 0);
        assert_eq!(l.pixel(100).pal, 2);
    }

    #[test]
    fn priorite_index() {
        let mut l = SpriteLine::default();
        l.slots[0] = plein(100, 0x01, true); // sprite 0, palette 1
        l.slots[1] = plein(100, 0x02, false); // palette 2
        l.count = 2;
        let p = l.pixel(103);
        assert_eq!((p.px, p.pal), (1, 1)); // l'emplacement 0 est visible
        assert!(p.sprite0);
    }

    #[test]
    fn transparent_laisse_passer() {
        let mut l = SpriteLine::default();
        l.slots[0] = SpriteSlot {
            pat_lo: 0x0F, // colonnes 4-7 seulement
            pat_hi: 0x00,
            attr: 0x01,
            x: 100,
            is_sprite0: true,
        };
        l.slots[1] = plein(100, 0x23, false); // derriere le fond, palette 3
        l.count = 2;
        let p = l.pixel(101); // colonne 1 : emplacement 0 transparent
        assert_eq!((p.px, p.pal, p.behind), (1, 3, true));
        assert!(!p.sprite0); // le sprite 0 n'est pas opaque ici
        assert!(l.pixel(105).sprite0);
    }

    #[test]
    fn count_limite_les_emplacements() {
        let mut l = SpriteLine::default();
        l.slots[3] = plein(10, 0, false);
        l.count = 3; // l'emplacement 3 est ignore
        assert_eq!(l.pixel(12).px, 0);
    }
}
