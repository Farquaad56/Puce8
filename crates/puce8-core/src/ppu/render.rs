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
            let couleur = self.couleur_pixel(x);
            self.framebuffer[usize::from(line) * 256 + x] = couleur;
        }
    }

    /// Valeur stockee pour le pixel x : index palette (0-5) | emphase (6-8).
    fn couleur_pixel(&self, x: usize) -> u16 {
        let mask = self.regs.mask;
        let index = if self.rendu_actif() {
            let (mut px, pal) = self.bg.pixel(self.regs.x);
            if mask & 0x08 == 0 || (x < 8 && mask & 0x02 == 0) {
                px = 0;
            }
            if px == 0 {
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
}
