//! Lecture memoire PPU sans effet de bord (E18e1) : pour les vues de debogage.
//! Aucun effet : ni `v`, ni `read_buffer`, ni `notify_ppu_address`.

use super::Ppu;
use crate::mapper::Mapper;

impl Ppu {
    /// Meme decodage que la lecture interne (E15b), sans effet de bord :
    /// `< $2000` -> `mapper.ppu_peek` ; `$2000-$3EFF` -> CIRAM (mirroring) ; `$3F00-$3FFF` -> palette.
    pub fn peek_vram(&self, addr: u16, mapper: &dyn Mapper) -> u8 {
        let a = addr & 0x3FFF;
        if a < 0x2000 {
            mapper.ppu_peek(a)
        } else if a < 0x3F00 {
            let n = usize::from((a >> 10) & 3);
            self.ciram[self.ciram_page(n, mapper) * 1024 + usize::from(a & 0x3FF)]
        } else {
            self.palette[Self::palette_index(a)] & 0x3F
        }
    }

    /// Registre $2000 (PPUCTRL).
    pub fn ctrl(&self) -> u8 {
        self.regs.ctrl
    }

    /// Defilement : (`t`, fine `x`).
    pub fn scroll_regs(&self) -> (u16, u8) {
        (self.regs.t, self.regs.x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::Mirroring;

    /// Mapper minimal : 8 Ko de CHR en RAM, mirroring vertical.
    struct Mem {
        mem: [u8; 0x2000],
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
            Mirroring::Vertical
        }
    }

    #[test]
    fn peek_vram_sans_effet() {
        let mut m = Mem { mem: [0; 0x2000] };
        m.mem[0x0010] = 0xAB;
        let mut ppu = Ppu::new();
        ppu.ciram[0] = 0x11;
        ppu.ciram[1] = 0x22;
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x16;
        // v = $2000, puis une lecture $2007 (tampon rempli avec $11, v = $2001).
        ppu.cpu_write_register(6, 0x20, &mut m);
        ppu.cpu_write_register(6, 0x00, &mut m);
        let _ = ppu.cpu_read_register(7, &mut m);
        let (v, tampon) = (ppu.regs.v, ppu.read_buffer);
        assert_eq!(ppu.peek_vram(0x0010, &m), 0xAB);
        assert_eq!(ppu.peek_vram(0x2800, &m), 0x11); // mirroring vertical : $2800 = $2000
        assert_eq!(ppu.peek_vram(0x3F01, &m), 0x16);
        assert_eq!(ppu.peek_vram(0x3F10, &m), 0x0F); // $3F10 = miroir de $3F00
        assert_eq!((ppu.regs.v, ppu.read_buffer), (v, tampon));
        assert_eq!(ppu.cpu_read_register(7, &mut m), 0x11); // la 2e lecture renvoie le tampon intact
    }

    #[test]
    fn scroll_regs_2005() {
        let mut m = Mem { mem: [0; 0x2000] };
        let mut ppu = Ppu::new();
        ppu.cpu_write_register(0, 0x10, &mut m);
        ppu.cpu_write_register(5, 0x7D, &mut m);
        ppu.cpu_write_register(5, 0x5E, &mut m);
        assert_eq!(ppu.ctrl(), 0x10);
        let (t, x) = ppu.scroll_regs();
        assert_eq!(x, 0x7D & 7);
        assert_eq!(t & 0x1F, 0x7D >> 3);
    }
}
