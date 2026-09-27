//! Pixels du fond et framebuffer (E18c3).

use super::Ppu;

impl Ppu {
    /// Rendu actif : fond ou sprites affiches.
    fn rendu_actif(&self) -> bool {
        self.regs.mask & 0x18 != 0
    }

    /// Travail du point courant, appele par `tick` AVANT `bg_fetch` :
    /// 1) pipeline des registres a decalage (decalage puis rechargement) ;
    /// 2) pixel (lignes 0-239, points 1-256).
    pub(super) fn render_dot(&mut self) {
        let (line, dot) = (self.line, self.point);
        if self.rendu_actif() && (line <= 239 || line == 261) {
            let (lo, hi, at) = (self.pat_lo_latch, self.pat_hi_latch, self.at_latch);
            self.bg.step(dot, lo, hi, at);
        }
        if line <= 239 && (1..=256).contains(&dot) {
            let x = usize::from(dot - 1);
            if self.sprite0_hit_at(x) {
                self.sprite0_hit = true; // E21c1
            }
            let couleur = self.couleur_pixel(x);
            self.framebuffer[usize::from(line) * 256 + x] = couleur;
        }
    }

    /// Pixel du fond (couleur 0-3, palette 0-3) au x donne, apres masquage
    /// (`mask & 0x08 == 0`, ou x < 8 avec `mask & 0x02 == 0` -> couleur 0).
    fn bg_pixel(&self, x: usize) -> (u8, u8) {
        let mask = self.regs.mask;
        let (px, pal) = self.bg.pixel(self.regs.x);
        if mask & 0x08 == 0 || (x < 8 && mask & 0x02 == 0) {
            (0, pal)
        } else {
            (px, pal)
        }
    }

    /// Sprite 0 hit au x donne (E21c1) : fond ET sprites affiches, x != 255, pixel du sprite 0
    /// opaque (meme s'il ne gagne pas la composition) ET pixel de fond opaque, apres masquages gauches.
    /// Independant de la priorite (`attr & 0x20`).
    fn sprite0_hit_at(&self, x: usize) -> bool {
        self.regs.mask & 0x18 == 0x18
            && x != 255
            && self.sprite_at(x).sprite0
            && self.bg_pixel(x).0 != 0
    }

    /// Valeur stockee pour le pixel x : index palette (0-5) | emphase (6-8).
    fn couleur_pixel(&self, x: usize) -> u16 {
        let mask = self.regs.mask;
        let index = if self.rendu_actif() {
            let (px, pal) = self.bg_pixel(x);
            let sp = self.sprite_at(x); // E21b2
            if sp.px != 0 && (!sp.behind || px == 0) {
                self.palette[usize::from(0x10 + sp.pal * 4 + sp.px)]
            } else if px == 0 {
                self.palette[0]
            } else {
                self.palette[usize::from(pal * 4 + px)]
            }
        } else {
            let v = self.regs.v;
            if (0x3F00..=0x3FFF).contains(&v) {
                self.palette[usize::from(v & 0x1F)]
            } else {
                self.palette[0]
            }
        };
        let index = if mask & 0x01 != 0 {
            index & 0x30
        } else {
            index & 0x3F
        };
        u16::from(index) | (u16::from(mask >> 5) << 6)
    }

    /// Increment de `v` apres un acces `$2007` (E22b1).
    /// Pendant le rendu (rendu actif, lignes 0-239 et 261) : `inc_coarse_x` ET `inc_y` en meme temps
    /// (le +1/+32 normal n'a pas lieu). Sinon : +1, ou +32 si `ctrl & 0x04`.
    /// SIMPLIFICATION: l'ecriture va quand meme a l'adresse `v` (revu en E36 si une ROM le demande).
    pub(super) fn incr_v_2007(&mut self) {
        if self.rendu_actif() && (self.line <= 239 || self.line == 261) {
            let v = super::background::inc_coarse_x(self.regs.v);
            self.regs.v = super::background::inc_y(v);
        } else {
            self.regs.incr_v();
        }
    }

    /// Image courante (256 x 240).
    pub fn frame_buffer(&self) -> &[u16] {
        &self.framebuffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::{Mapper, Mirroring};

    /// 8 Ko de CHR en RAM, mirroring horizontal.
    struct Chr {
        mem: [u8; 0x2000],
    }

    impl Mapper for Chr {
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
        fn notify_ppu_address(&mut self, _addr: u16) {}
    }

    /// Tuile 1 = motif bas FF / haut 00 ; nametable 0 remplie de 1 ; palette[0] = $0F, palette[1] = $16.
    /// Demarre en (261, 0) (pre-render) et joue jusqu'a (240, 0).
    fn image(mask: u8) -> Ppu {
        let mut chr = Chr { mem: [0; 0x2000] };
        for i in 0..8 {
            chr.mem[16 + i] = 0xFF;
        }
        let mut ppu = Ppu::new();
        ppu.ciram[..960].fill(1);
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x16;
        ppu.regs.mask = mask;
        ppu.line = 261;
        ppu.point = 0;
        while ppu.position() != (240, 0) {
            ppu.tick(&mut chr);
        }
        ppu
    }

    #[test]
    fn pixel_tuile_pleine() {
        let ppu = image(0x0A);
        assert_eq!(ppu.frame_buffer()[10 * 256 + 10], 0x16);
        assert_eq!(ppu.frame_buffer()[0], 0x16);
    }

    #[test]
    fn clip_gauche() {
        let ppu = image(0x08);
        for x in 0..8 {
            assert_eq!(ppu.frame_buffer()[20 * 256 + x], 0x0F, "x = {x}");
        }
        assert_eq!(ppu.frame_buffer()[20 * 256 + 8], 0x16);
    }

    #[test]
    fn rendu_inactif_backdrop() {
        let mut ppu = image(0x00);
        assert_eq!(ppu.frame_buffer()[100 * 256 + 100], 0x0F);
        ppu.palette[5] = 0x21;
        ppu.regs.v = 0x3F05;
        ppu.line = 50;
        ppu.point = 0;
        let mut chr = Chr { mem: [0; 0x2000] };
        ppu.tick(&mut chr);
        assert_eq!(ppu.frame_buffer()[50 * 256], 0x21);
    }

    #[test]
    fn gris_et_emphase() {
        let ppu = image(0x0B | 0x20); // gris + emphase rouge
        assert_eq!(ppu.frame_buffer()[10 * 256 + 10], 0x10 | (1 << 6));
    }

    // ---------- E22b1 : $2007 pendant le rendu ----------

    #[test]
    fn double_increment_2007() {
        let mut chr = Chr { mem: [0; 0x2000] };
        let mut ppu = Ppu::new();
        ppu.regs.mask = 0x18;
        ppu.line = 100; // ligne visible, rendu actif
        ppu.regs.v = 0x1005; // fine Y = 1, coarse X = 5
        let _ = ppu.cpu_read_register(7, &mut chr);
        assert_eq!(ppu.regs.v, 0x2006); // coarse X + 1 ET fine Y + 1
        ppu.cpu_write_register(7, 0xAB, &mut chr);
        assert_eq!(ppu.regs.v, 0x3007);
    }

    #[test]
    fn increment_normal_hors_rendu() {
        let mut chr = Chr { mem: [0; 0x2000] };
        let mut ppu = Ppu::new();
        ppu.regs.mask = 0x18;
        ppu.line = 245; // VBlank : increment normal
        ppu.regs.v = 0x1005;
        let _ = ppu.cpu_read_register(7, &mut chr);
        assert_eq!(ppu.regs.v, 0x1006);
        ppu.line = 100;
        ppu.regs.mask = 0x00; // rendu coupe : increment normal
        ppu.regs.ctrl = 0x04; // +32
        ppu.cpu_write_register(7, 0xAB, &mut chr);
        assert_eq!(ppu.regs.v, 0x1026);
    }
}
