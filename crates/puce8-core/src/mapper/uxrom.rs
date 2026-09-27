//! UxROM (mapper 2) : PRG banke par blocs de 16 Ko, CHR en general en RAM.
// wiki: UxROM ; wiki: INES_Mapper_002

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_RAM_MIN: usize = 8 * 1024;
const BANK_SIZE: usize = 16 * 1024;

/// Cartouche UxROM : $8000-$BFFF = banque choisie, $C000-$FFFF = derniere banque.
pub struct Uxrom {
    prg_rom: Vec<u8>,
    /// Registre de selection (toute ecriture $8000-$FFFF le pose).
    bank: u8,
    /// $6000-$7FFF, toujours fournie ; miroir si `prg_ram_size` < 8 Ko.
    prg_ram: Vec<u8>,
    chr: ChrMemory,
    mirroring: Mirroring,
    has_battery: bool,
}

impl Uxrom {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        // PRG-RAM $6000 comme pour NROM.
        let prg_ram = vec![0u8; cart.prg_ram_size.max(PRG_RAM_MIN)];
        Uxrom {
            prg_rom: cart.prg_rom,
            bank: 0,
            prg_ram,
            chr,
            mirroring: cart.mirroring,
            has_battery: cart.has_battery,
        }
    }

    /// Nombre de banques de 16 Ko (minimum 1).
    fn nb_banks(&self) -> usize {
        (self.prg_rom.len() / BANK_SIZE).max(1)
    }

    /// $8000-$BFFF = banque `bank % nb` ; $C000-$FFFF = derniere banque.
    // SIMPLIFICATION: pas de conflits de bus.
    fn prg_read(&self, addr: u16) -> Option<u8> {
        if !(0x8000..=0xFFFF).contains(&addr) || self.prg_rom.is_empty() {
            return None;
        }
        let nb = self.nb_banks();
        let bank_idx = if addr >= 0xC000 {
            nb - 1
        } else {
            (self.bank as usize) % nb
        };
        let off = (addr & 0x3FFF) as usize; // offset dans la banque de 16 Ko
        self.prg_rom.get(bank_idx * BANK_SIZE + off).copied()
    }

    /// $6000-$7FFF : PRG-RAM, miroir si plus courte que 8 Ko.
    fn ram_read(&self, addr: u16) -> Option<u8> {
        if (0x6000..=0x7FFF).contains(&addr) {
            let offset = (addr - 0x6000) as usize % self.prg_ram.len();
            return Some(self.prg_ram[offset]);
        }
        None
    }

    /// Lecture sans effet de bord : meme valeur que `cpu_read`.
    fn read_at(&self, addr: u16) -> Option<u8> {
        // $4020-$5FFF : rien de branche -> open bus (None).
        if (0x4020..=0x7FFF).contains(&addr) {
            return self.ram_read(addr);
        }
        self.prg_read(addr)
    }
}

impl Mapper for Uxrom {
    fn cpu_read(&mut self, addr: u16) -> Option<u8> {
        self.read_at(addr)
    }

    fn cpu_peek(&self, addr: u16) -> Option<u8> {
        self.read_at(addr) // aucune lecture n'a d'effet de bord
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let offset = (addr - 0x6000) as usize % self.prg_ram.len();
            self.prg_ram[offset] = value;
        } else if (0x8000..=0xFFFF).contains(&addr) {
            // Toute ecriture $8000-$FFFF pose le registre de banque.
            self.bank = value;
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr.read(addr as usize) // CHR non banke (generalement en RAM)
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        self.chr.write(addr as usize, value);
    }

    fn ppu_peek(&self, addr: u16) -> u8 {
        self.chr.read(addr as usize) // lecture CHR sans effet de bord
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

    /// ROM synthetique UxROM : PRG de `prg_banks` banques, valeur = numero de banque.
    fn make_rom(prg_banks: u8) -> Vec<u8> {
        let mut rom = vec![b'N', b'E', b'S', 0x1A, prg_banks, 0, 0x20, 0]; // mapper 2, horizontal
        rom.extend_from_slice(&[0; 8]); // bytes 8-15 de l'en-tete (propres)
        let prg_len = usize::from(prg_banks) * BANK_SIZE;
        rom.extend((0..prg_len).map(|i| (i / BANK_SIZE) as u8));
        rom
    }

    fn cart(prg_banks: u8) -> Cartridge {
        Cartridge::from_bytes(&make_rom(prg_banks)).unwrap()
    }

    #[test]
    fn uxrom_fixe() {
        let mut m = Uxrom::new(cart(8));
        // $C000-$FFFF : toujours la derniere banque (7 sur 8).
        assert_eq!(m.cpu_read(0xC000), Some(7));
        assert_eq!(m.cpu_read(0xFFFF), Some(7));
    }

    #[test]
    fn uxrom_commute() {
        let mut m = Uxrom::new(cart(8));
        m.cpu_write(0x9000, 3); // ecriture $8000-$FFFF -> bank = 3
        assert_eq!(m.cpu_read(0x8000), Some(3));
    }

    #[test]
    fn uxrom_modulo() {
        let mut m = Uxrom::new(cart(8));
        m.cpu_write(0x9000, 9); // 9 % 8 = 1
        assert_eq!(m.cpu_read(0x8000), Some(1));
    }
}
