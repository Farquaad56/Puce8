//! Vues de debogage (E18e2/E18e3) : table des motifs, nametables, defilement.
//! Lecture seule : uniquement `Ppu::peek_vram` (aucun effet de bord).
//! Sorties = indices de palette u16 (meme format que le framebuffer) -> `to_rgba`, `frame_hash`.

use crate::mapper::Mapper;
use crate::ppu::Ppu;

/// Table des motifs : $0000 a gauche (x 0-127), $1000 a droite (x 128-255).
pub const PATTERNS_W: usize = 256;
pub const PATTERNS_H: usize = 128;

/// Palette d'affichage de la table des motifs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPalette {
    /// Niveaux de gris fixes : [$0F, $00, $10, $30].
    Gray,
    /// Palette 0-3 (fond) ou 4-7 (sprites).
    Index(u8),
}

/// Couleur 0-3 du pixel (row, col) d'une tuile dont les 16 octets commencent en `addr`.
fn tile_px(ppu: &Ppu, mapper: &dyn Mapper, addr: u16, row: u16, col: u16) -> u8 {
    let lo = ppu.peek_vram(addr + row, mapper);
    let hi = ppu.peek_vram(addr + row + 8, mapper);
    (((hi >> (7 - col)) & 1) << 1) | ((lo >> (7 - col)) & 1)
}

/// Index palette de la couleur `px` (0-3) dans la palette `pal` (0-7).
fn pal_color(ppu: &Ppu, mapper: &dyn Mapper, pal: u8, px: u8) -> u16 {
    let addr = if px == 0 {
        0x3F00
    } else {
        0x3F00 + u16::from(pal & 7) * 4 + u16::from(px)
    };
    u16::from(ppu.peek_vram(addr, mapper))
}

/// Dessine les 2 tables de motifs (512 tuiles) dans `out` (PATTERNS_W x PATTERNS_H).
pub fn render_patterns(ppu: &Ppu, mapper: &dyn Mapper, pal: ViewPalette, out: &mut [u16]) {
    const GRAY: [u16; 4] = [0x0F, 0x00, 0x10, 0x30];
    for table in 0..2u16 {
        for t in 0..256u16 {
            let addr = table * 0x1000 + t * 16;
            let x0 = usize::from(table * 128 + (t % 16) * 8);
            let y0 = usize::from((t / 16) * 8);
            for row in 0..8u16 {
                for col in 0..8u16 {
                    let px = tile_px(ppu, mapper, addr, row, col);
                    let color = match pal {
                        ViewPalette::Gray => GRAY[usize::from(px)],
                        ViewPalette::Index(p) => pal_color(ppu, mapper, p, px),
                    };
                    out[(y0 + usize::from(row)) * PATTERNS_W + x0 + usize::from(col)] = color;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::Mirroring;

    /// 8 Ko de CHR en RAM ; mirroring choisi par le test.
    struct Mem {
        mem: [u8; 0x2000],
        mir: Mirroring,
    }

    impl Mapper for Mem {
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
            self.mir
        }
    }

    fn mem(mir: Mirroring) -> Mem {
        Mem {
            mem: [0; 0x2000],
            mir,
        }
    }

    #[test]
    fn patterns_tuile_1() {
        let mut m = mem(Mirroring::Horizontal);
        for r in 0..8 {
            m.mem[16 + r] = 0xFF; // tuile 1 : plan bas = FF, plan haut = 00 -> couleur 1
        }
        let mut ppu = Ppu::new();
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x16;
        let mut out = vec![0u16; PATTERNS_W * PATTERNS_H];
        render_patterns(&ppu, &m, ViewPalette::Index(0), &mut out);
        assert_eq!(out[8], 0x16); // (8, 0) : tuile 1
        assert_eq!(out[0], 0x0F); // (0, 0) : tuile 0 vide -> $3F00
        render_patterns(&ppu, &m, ViewPalette::Gray, &mut out);
        assert_eq!(out[8], 0x00); // gris : couleur 1 -> $00
        assert_eq!(out[0], 0x0F);
    }

    #[test]
    fn patterns_table_1() {
        let mut m = mem(Mirroring::Horizontal);
        for r in 0..8 {
            m.mem[0x1000 + r] = 0xFF; // tuile 0 de la table $1000
        }
        let ppu = Ppu::new();
        let mut out = vec![0u16; PATTERNS_W * PATTERNS_H];
        render_patterns(&ppu, &m, ViewPalette::Gray, &mut out);
        assert_eq!(out[128], 0x00); // x = 128 : table de droite, couleur 1
        assert_eq!(out[127], 0x0F); // x = 127 : table de gauche, vide
    }
}
