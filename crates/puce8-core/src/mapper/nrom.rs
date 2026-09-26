//! NROM (mapper 0) : PRG-ROM fixe, PRG-RAM toujours fournie, CHR-ROM ou CHR-RAM.
// wiki: NROM ; wiki: INES_Mapper_000

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_RAM_MIN: usize = 8 * 1024;

/// Cartouche NROM-128 (PRG 16 Ko) ou NROM-256 (PRG 32 Ko).
pub struct Nrom {
    prg_rom: Vec<u8>,
    /// $6000-$7FFF, toujours fournie ; miroir si `prg_ram_size` < 8 Ko.
    prg_ram: Vec<u8>,
    chr: ChrMemory,
    mirroring: Mirroring,
    has_battery: bool,
}

impl Nrom {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        // TESTS par. T3.b : les ROM de test blargg ecrivent leurs resultats en $6000.
        let prg_ram = vec![0u8; cart.prg_ram_size.max(PRG_RAM_MIN)];
        Nrom {
            chr,
            prg_rom: cart.prg_rom,
            prg_ram,
            mirroring: cart.mirroring,
            has_battery: cart.has_battery,
        }
    }

    /// $8000-$FFFF : PRG-ROM, miroir si plus courte que 16 Ko.
    // wiki: NROM (PRG-ROM)
    fn prg_read(&self, addr: u16) -> Option<u8> {
        if addr >= 0x8000 && !self.prg_rom.is_empty() {
            let offset = (addr - 0x8000) as usize % self.prg_rom.len();
            return Some(self.prg_rom[offset]);
        }
        None
    }

    /// $6000-$7FFF : PRG-RAM, miroir si plus courte que 8 Ko.
    fn ram_read(&self, addr: u16) -> Option<u8> {
        if (0x6000..=0x7FFF).contains(&addr) {
            let offset = (addr - 0x6000) as usize % self.prg_ram.len();
            return Some(self.prg_ram[offset]);
        }
        None
    }
}

impl Mapper for Nrom {
    fn cpu_read(&mut self, addr: u16) -> Option<u8> {
        // $4020-$5FFF : rien de branche -> open bus (None).
        if (0x4020..=0x7FFF).contains(&addr) {
            return self.ram_read(addr);
        }
        self.prg_read(addr)
    }

    fn cpu_peek(&self, addr: u16) -> Option<u8> {
        // NROM : aucune lecture n'a d'effet de bord -> identique a `cpu_read`.
        if (0x4020..=0x7FFF).contains(&addr) {
            return self.ram_read(addr);
        }
        self.prg_read(addr)
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let offset = (addr - 0x6000) as usize % self.prg_ram.len();
            self.prg_ram[offset] = value;
        }
        // $8000-$FFFF : PRG-ROM, ecritures ignorees.
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr.read(addr as usize)
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        self.chr.write(addr as usize, value);
    }

    fn ppu_peek(&self, addr: u16) -> u8 {
        self.chr.read(addr as usize) // NROM : lecture CHR sans effet de bord
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn battery_ram(&self) -> Option<&[u8]> {
        if self.has_battery {
            Some(self.prg_ram.as_slice())
        } else {
            None
        }
    }

    fn load_battery_ram(&mut self, data: &[u8]) {
        let n = data.len().min(self.prg_ram.len());
        self.prg_ram[..n].copy_from_slice(&data[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM synthetique : PRG = `i % 256`, CHR = `0xC0 | (i % 64)`.
    fn make_rom(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut rom = vec![b'N', b'E', b'S', 0x1A, prg_banks, chr_banks, flags6, flags7];
        rom.extend_from_slice(&[0; 8]);
        let prg_len = usize::from(prg_banks) * 16_384;
        rom.extend((0..prg_len).map(|i| (i % 256) as u8));
        let chr_len = usize::from(chr_banks) * 8_192;
        rom.extend((0..chr_len).map(|i| 0xC0 | (i % 64) as u8));
        rom
    }

    fn cart(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Cartridge {
        Cartridge::from_bytes(&make_rom(prg_banks, chr_banks, flags6, flags7)).unwrap()
    }

    #[test]
    fn nrom128_miroir() {
        let mut m = Nrom::new(cart(1, 1, 0, 0));
        assert_eq!(m.cpu_read(0x8000), Some(0));
        assert_eq!(m.cpu_read(0x8000), m.cpu_read(0xC000));
        assert_eq!(m.cpu_read(0xBFFF), m.cpu_read(0xFFFF));
        // cpu_peek : meme valeur que cpu_read, sans effet de bord.
        assert_eq!(m.cpu_peek(0xC000), m.cpu_read(0xC000));
    }

    #[test]
    fn nrom256_pas_de_miroir() {
        let mut rom = make_rom(2, 1, 0, 0);
        rom[16 + 0x4000] = 0x77; // $C000 dans une PRG de 32 Ko
        let mut m = Nrom::new(Cartridge::from_bytes(&rom).unwrap());
        assert_eq!(m.cpu_read(0xC000), Some(0x77));
    }

    #[test]
    fn prg_ram_rw() {
        let mut m = Nrom::new(cart(1, 1, 0, 0));
        m.cpu_write(0x6000, 0x42);
        assert_eq!(m.cpu_read(0x6000), Some(0x42));
        m.cpu_write(0x7FFF, 0xAB);
        assert_eq!(m.cpu_read(0x7FFF), Some(0xAB));
    }

    #[test]
    fn rom_non_modifiable() {
        let mut m = Nrom::new(cart(1, 1, 0, 0));
        let avant = m.cpu_read(0x8000);
        m.cpu_write(0x8000, 0xFF);
        assert_eq!(m.cpu_read(0x8000), avant);
    }

    #[test]
    fn open_bus_4020() {
        let mut m = Nrom::new(cart(1, 1, 0, 0));
        assert_eq!(m.cpu_read(0x5000), None);
        assert_eq!(m.cpu_read(0x4020), None);
        assert_eq!(m.cpu_read(0x5FFF), None);
    }

    #[test]
    fn chr_rom_lecture_seule() {
        let mut m = Nrom::new(cart(1, 1, 0, 0));
        let avant = m.ppu_read(0x0000);
        m.ppu_write(0x0000, 0xFF);
        assert_eq!(m.ppu_read(0x0000), avant);
    }

    #[test]
    fn chr_ram_rw() {
        let mut m = Nrom::new(cart(1, 0, 0, 0)); // pas de CHR-ROM -> CHR-RAM
        m.ppu_write(0x1234, 0x5A);
        assert_eq!(m.ppu_read(0x1234), 0x5A);
    }

    #[test]
    fn mirroring_entete() {
        let m = Nrom::new(cart(1, 1, 0x01, 0)); // bit 0 de f6 -> vertical
        assert_eq!(m.mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn battery_ram() {
        let m = Nrom::new(cart(1, 1, 0, 0));
        assert!(m.battery_ram().is_none());
        let mut mb = Nrom::new(cart(1, 1, 0x02, 0)); // bit 1 de f6 -> batterie
        let ram = mb.battery_ram().expect("batterie presente");
        assert_eq!(ram.len(), PRG_RAM_MIN);
        mb.load_battery_ram(&[0xDE, 0xAD]);
        let ram = mb.battery_ram().unwrap();
        assert_eq!(&ram[..2], &[0xDE, 0xAD]);
    }
}
