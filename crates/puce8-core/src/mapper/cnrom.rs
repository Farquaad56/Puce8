//! CNROM (mapper 3) : PRG fixe comme NROM, CHR banke par blocs de 8 Ko.
// wiki: CNROM ; wiki: INES_Mapper_003

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_RAM_MIN: usize = 8 * 1024;
const CHR_BANK_SIZE: usize = 8 * 1024;

/// Cartouche CNROM : PRG-ROM fixe, selection de banque CHR par ecriture $8000-$FFFF.
pub struct Cnrom {
    prg_rom: Vec<u8>,
    /// Banque CHR choisie (deja reduite modulo le nombre de banques).
    chr_bank: u8,
    /// $6000-$7FFF, toujours fournie ; miroir si `prg_ram_size` < 8 Ko.
    prg_ram: Vec<u8>,
    chr: ChrMemory,
    mirroring: Mirroring,
    has_battery: bool,
}

impl Cnrom {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        // PRG-RAM $6000 comme pour NROM.
        let prg_ram = vec![0u8; cart.prg_ram_size.max(PRG_RAM_MIN)];
        Cnrom {
            prg_rom: cart.prg_rom,
            chr_bank: 0,
            prg_ram,
            chr,
            mirroring: cart.mirroring,
            has_battery: cart.has_battery,
        }
    }

    /// Nombre de banques CHR de 8 Ko (minimum 1).
    fn nb_chr_banks(&self) -> usize {
        (self.chr.len() / CHR_BANK_SIZE).max(1)
    }

    /// $8000-$FFFF : PRG-ROM fixe, miroir si plus courte que 16 Ko.
    // SIMPLIFICATION: pas de conflits de bus.
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

    /// Lecture sans effet de bord : meme valeur que `cpu_read`.
    fn read_at(&self, addr: u16) -> Option<u8> {
        // $4020-$5FFF : rien de branche -> open bus (None).
        if (0x4020..=0x7FFF).contains(&addr) {
            return self.ram_read(addr);
        }
        self.prg_read(addr)
    }

    /// Octet CHR a l'adresse PPU donnee, dans la banque `chr_bank`.
    fn chr_byte(&self, addr: u16) -> u8 {
        let off_in_bank = (addr & 0x1FFF) as usize; // offset dans la banque de 8 Ko
        let bank_idx = (self.chr_bank as usize) % self.nb_chr_banks();
        self.chr.read(bank_idx * CHR_BANK_SIZE + off_in_bank)
    }

    /// Offset global CHR a l'adresse PPU donnee, dans la banque `chr_bank`.
    fn chr_offset(&self, addr: u16) -> usize {
        let off_in_bank = (addr & 0x1FFF) as usize;
        let bank_idx = (self.chr_bank as usize) % self.nb_chr_banks();
        bank_idx * CHR_BANK_SIZE + off_in_bank
    }
}

impl Mapper for Cnrom {
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
            // Ecriture $8000-$FFFF -> selection de banque CHR.
            let nb = self.nb_chr_banks();
            self.chr_bank = (value as usize % nb) as u8;
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr_byte(addr) // CHR banke par blocs de 8 Ko
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        self.chr.write(self.chr_offset(addr), value);
    }

    fn ppu_peek(&self, addr: u16) -> u8 {
        self.chr_byte(addr) // lecture CHR sans effet de bord
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

    /// ROM synthetique CNROM : PRG fixe, CHR de `chr_banks` banques (valeur = numero).
    fn make_rom(prg_banks: u8, chr_banks: u8) -> Vec<u8> {
        let mut rom = vec![b'N', b'E', b'S', 0x1A, prg_banks, chr_banks, 0x30, 0]; // mapper 3, horizontal
        rom.extend_from_slice(&[0; 8]); // bytes 8-15 de l'en-tete (propres)
        let prg_len = usize::from(prg_banks) * 16_384;
        rom.extend((0..prg_len).map(|i| (i / 16_384) as u8));
        let chr_len = usize::from(chr_banks) * CHR_BANK_SIZE;
        rom.extend((0..chr_len).map(|i| (i / CHR_BANK_SIZE) as u8)); // valeur = numero de banque
        rom
    }

    fn cart(prg_banks: u8, chr_banks: u8) -> Cartridge {
        Cartridge::from_bytes(&make_rom(prg_banks, chr_banks)).unwrap()
    }

    #[test]
    fn cnrom_chr() {
        let mut m = Cnrom::new(cart(1, 4));
        assert_eq!(m.ppu_read(0x0000), 0); // banque 0 par defaut
        m.cpu_write(0x8000, 2); // ecriture $8000-$FFFF -> chr_bank = 2
        assert_eq!(m.ppu_read(0x0000), 2); // PPU $0000 lit la banque 2
    }
}
