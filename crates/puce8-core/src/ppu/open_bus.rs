//! Open bus de la PPU avec decroissance (E22a1) : registre de 8 bits dont chaque bit a 1
//! retombe a 0 s'il n'a pas ete rafraichi depuis environ 600 ms (readme de `ppu_open_bus`).
//! Structure PURE : l'image courante (`frame`) est passee en parametre.

/// Nombre d'images sans rafraichissement avant qu'un bit a 1 retombe a 0 (~600 ms a 60 Hz).
pub const DECAY_FRAMES: u64 = 36;

/// Registre de decroissance : valeur + image du dernier rafraichissement de chaque bit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DecayLatch {
    value: u8,
    stamp: [u64; 8],
}

impl DecayLatch {
    /// Ecriture dans un registre PPU : les 8 bits prennent la valeur ecrite.
    pub fn write(&mut self, v: u8, frame: u64) {
        self.refresh(0xFF, v, frame);
    }

    /// Rafraichit les bits de `mask` avec ceux de `v` (lecture : bits definis par la PPU).
    pub fn refresh(&mut self, mask: u8, v: u8, frame: u64) {
        for bit in 0..8 {
            if mask & (1 << bit) != 0 {
                self.stamp[bit] = frame;
            }
        }
        self.value = (self.value & !mask) | (v & mask);
    }

    /// Valeur a l'image `frame` : les bits a 1 non rafraichis depuis DECAY_FRAMES images valent 0.
    pub fn value(&self, frame: u64) -> u8 {
        let mut out = self.value;
        for bit in 0..8 {
            if frame.saturating_sub(self.stamp[bit]) >= DECAY_FRAMES {
                out &= !(1 << bit);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decroissance() {
        let mut l = DecayLatch::default();
        l.write(0xFF, 100);
        assert_eq!(l.value(100 + 40), 0x00);
    }

    #[test]
    fn rafraichi() {
        let mut l = DecayLatch::default();
        l.write(0xFF, 100);
        assert_eq!(l.value(110), 0xFF);
        assert_eq!(l.value(100 + DECAY_FRAMES - 1), 0xFF);
        assert_eq!(l.value(100 + DECAY_FRAMES), 0x00);
    }

    #[test]
    fn rafraichissement_partiel() {
        let mut l = DecayLatch::default();
        l.write(0xFF, 0);
        l.refresh(0xE0, 0xA0, 30); // bits 7-5 rafraichis avec 101
        assert_eq!(l.value(30), 0xBF); // 101 + anciens bits 4-0
        assert_eq!(l.value(40), 0xA0); // bits 4-0 retombes (image 0 + 36), bits 7 et 5 encore a 1
        assert_eq!(l.value(30 + DECAY_FRAMES), 0x00);
    }

    // ---------- E22a2 : open bus branche dans le Ppu ----------

    use crate::bus::Bus;
    use crate::ppu::Ppu;

    fn ppu_bus() -> (Ppu, Bus) {
        (Ppu::new(), Bus::for_test_with_prg(&[0xA9]))
    }

    #[test]
    fn ppu_decroissance_2000() {
        let (mut ppu, mut bus) = ppu_bus();
        ppu.cpu_write_register(0, 0xFF, &mut (*bus.mapper));
        ppu.frame += 10;
        assert_eq!(ppu.cpu_read_register(0, &mut (*bus.mapper)), 0xFF); // rafraichi recemment
        ppu.frame += 30; // 40 images apres l'ecriture
        assert_eq!(ppu.cpu_read_register(0, &mut (*bus.mapper)), 0x00);
    }

    #[test]
    fn bits_2002() {
        let (mut ppu, mut bus) = ppu_bus();
        ppu.cpu_write_register(1, 0x1F, &mut (*bus.mapper)); // open bus = $1F
        ppu.vblank = true;
        assert_eq!(ppu.cpu_read_register(2, &mut (*bus.mapper)), 0x9F); // VBlank + bits 4-0 open bus
        ppu.frame += 40;
        assert_eq!(ppu.cpu_read_register(2, &mut (*bus.mapper)) & 0x1F, 0x00); // bits 4-0 retombes
    }

    #[test]
    fn lecture_2002_rafraichit_bit_7() {
        let (mut ppu, mut bus) = ppu_bus();
        ppu.cpu_write_register(0, 0x00, &mut (*bus.mapper));
        ppu.vblank = true;
        let _ = ppu.cpu_read_register(2, &mut (*bus.mapper)); // bit 7 = 1 rafraichit l'open bus
        assert_eq!(ppu.cpu_read_register(0, &mut (*bus.mapper)), 0x80);
    }

    #[test]
    fn lecture_2007_palette_bits_hauts() {
        let (mut ppu, mut bus) = ppu_bus();
        ppu.palette[0] = 0x0F;
        ppu.cpu_write_register(6, 0x3F, &mut (*bus.mapper));
        ppu.cpu_write_register(6, 0x00, &mut (*bus.mapper)); // open bus = $00 ; v = $3F00
        ppu.cpu_write_register(3, 0xC0, &mut (*bus.mapper)); // open bus = $C0
        assert_eq!(ppu.cpu_read_register(7, &mut (*bus.mapper)), 0xCF); // $0F | bits 7-6 open bus
    }
}
