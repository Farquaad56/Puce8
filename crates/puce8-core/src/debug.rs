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

/// Les 4 nametables : NT0 NT1 en haut, NT2 NT3 en bas.
pub const NAMETABLES_W: usize = 512;
pub const NAMETABLES_H: usize = 480;

/// Base des motifs du fond : $1000 si `ctrl & 0x10`, sinon $0000.
fn bg_base(ppu: &Ppu) -> u16 {
    if ppu.ctrl() & 0x10 != 0 {
        0x1000
    } else {
        0x0000
    }
}

/// Palette 0-3 de la tuile (cx, cy) de la nametable `base` (octet d'attribut + quadrant).
fn tile_palette(ppu: &Ppu, mapper: &dyn Mapper, base: u16, cx: u16, cy: u16) -> u8 {
    let attr = ppu.peek_vram(base + 0x3C0 + (cy / 4) * 8 + cx / 4, mapper);
    (attr >> (((cy & 2) << 1) | (cx & 2))) & 3
}

/// Dessine les 4 nametables dans `out` (NAMETABLES_W x NAMETABLES_H), avec le mirroring du mapper.
pub fn render_nametables(ppu: &Ppu, mapper: &dyn Mapper, out: &mut [u16]) {
    let pat = bg_base(ppu);
    for n in 0..4u16 {
        let base = 0x2000 + n * 0x400;
        let ox = usize::from((n & 1) * 256);
        let oy = usize::from((n >> 1) * 240);
        for cy in 0..30u16 {
            for cx in 0..32u16 {
                let idx = u16::from(ppu.peek_vram(base + cy * 32 + cx, mapper));
                let pal = tile_palette(ppu, mapper, base, cx, cy);
                for row in 0..8u16 {
                    for col in 0..8u16 {
                        let px = tile_px(ppu, mapper, pat + idx * 16, row, col);
                        let x = ox + usize::from(cx * 8 + col);
                        let y = oy + usize::from(cy * 8 + row);
                        out[y * NAMETABLES_W + x] = pal_color(ppu, mapper, pal, px);
                    }
                }
            }
        }
    }
}

/// Coin haut-gauche de l'ecran dans l'image des nametables (512 x 480), depuis `t` et fine `x`.
pub fn scroll_origin(ppu: &Ppu) -> (u16, u16) {
    let (t, x) = ppu.scroll_regs();
    let sx = (t & 0x1F) * 8 + u16::from(x) + ((t >> 10) & 1) * 256;
    let sy = (((t >> 5) & 0x1F) * 8 + ((t >> 12) & 7) + ((t >> 11) & 1) * 240) % 480;
    (sx, sy)
}

/// Informations sur la tuile sous le point (x, y) de l'image des nametables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileInfo {
    pub nt_addr: u16,
    pub tile: u8,
    pub attr_addr: u16,
    pub palette: u8,
    pub chr_addr: u16,
}

/// Tuile sous (x, y), avec x < 512 et y < 480.
pub fn tile_at(ppu: &Ppu, mapper: &dyn Mapper, x: u16, y: u16) -> TileInfo {
    let n = (y / 240) * 2 + x / 256;
    let base = 0x2000 + n * 0x400;
    let (cx, cy) = ((x % 256) / 8, (y % 240) / 8);
    let nt_addr = base + cy * 32 + cx;
    let tile = ppu.peek_vram(nt_addr, mapper);
    TileInfo {
        nt_addr,
        tile,
        attr_addr: base + 0x3C0 + (cy / 4) * 8 + cx / 4,
        palette: tile_palette(ppu, mapper, base, cx, cy),
        chr_addr: bg_base(ppu) + u16::from(tile) * 16,
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

    /// Tuile 1 = couleur 1 partout ; palettes : $3F00 = 0F, $3F01 = 16, $3F05 = 26, $3F09 = 36, $3F0D = 06.
    fn ppu_et_chr(mir: Mirroring) -> (Ppu, Mem) {
        let mut m = mem(mir);
        for r in 0..8 {
            m.mem[16 + r] = 0xFF;
        }
        let mut ppu = Ppu::new();
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x16;
        ppu.palette[5] = 0x26;
        ppu.palette[9] = 0x36;
        ppu.palette[13] = 0x06;
        (ppu, m)
    }

    #[test]
    fn nametable_miroir_vertical() {
        let (mut ppu, m) = ppu_et_chr(Mirroring::Vertical);
        ppu.ciram[0] = 1; // tuile 1 en $2000 (page 0)
        let mut out = vec![0u16; NAMETABLES_W * NAMETABLES_H];
        render_nametables(&ppu, &m, &mut out);
        assert_eq!(out[0], 0x16); // (0, 0) : NT0
        assert_eq!(out[240 * NAMETABLES_W], 0x16); // (0, 240) : NT2 = NT0 en vertical
        assert_eq!(out[256], 0x0F); // (256, 0) : NT1 = page 1, vide
    }

    #[test]
    fn attribut_quadrants() {
        let (mut ppu, m) = ppu_et_chr(Mirroring::Horizontal);
        for (cx, cy) in [(0usize, 0usize), (2, 0), (0, 2), (2, 2)] {
            ppu.ciram[cy * 32 + cx] = 1;
        }
        ppu.ciram[0x3C0] = 0b11_10_01_00;
        let mut out = vec![0u16; NAMETABLES_W * NAMETABLES_H];
        render_nametables(&ppu, &m, &mut out);
        let px = |cx: usize, cy: usize| out[cy * 8 * NAMETABLES_W + cx * 8];
        assert_eq!(px(0, 0), 0x16); // pal 0
        assert_eq!(px(2, 0), 0x26); // pal 1
        assert_eq!(px(0, 2), 0x36); // pal 2
        assert_eq!(px(2, 2), 0x06); // pal 3
        assert_eq!(tile_at(&ppu, &m, 16, 16).palette, 3);
    }

    #[test]
    fn scroll_origin_2005() {
        let (mut ppu, mut m) = ppu_et_chr(Mirroring::Horizontal);
        ppu.cpu_write_register(5, 0x7D, &mut m);
        ppu.cpu_write_register(5, 0x5E, &mut m);
        assert_eq!(scroll_origin(&ppu), (125, 94));
    }

    #[test]
    fn tile_at_simple() {
        let (mut ppu, m) = ppu_et_chr(Mirroring::Horizontal);
        ppu.ciram[1] = 1;
        let info = tile_at(&ppu, &m, 8, 0);
        assert_eq!(info.nt_addr, 0x2001);
        assert_eq!(info.tile, 1);
        assert_eq!(info.attr_addr, 0x23C0);
        assert_eq!(info.chr_addr, 0x0010);
        assert_eq!(tile_at(&ppu, &m, 256, 240).nt_addr, 0x2C00);
    }
}
