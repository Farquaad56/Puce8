//! MMC3 (mapper 4) : banques PRG 8 Ko / CHR 1 Ko, mirroring, PRG-RAM (E27a) ; IRQ de ligne (E28a).
// wiki: MMC3 ; wiki: INES_Mapper_004

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_BANK: usize = 8 * 1024;
const PRG_RAM_MIN: usize = 8 * 1024;

/// Cartouche MMC3 (TxROM).
pub struct Mmc3 {
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr: ChrMemory,
    /// $8000 : `CPMx xRRR` (R = registre vise, P = mode PRG, C = inversion CHR).
    bank_select: u8,
    /// R0-R7, ecrits par $8001.
    regs: [u8; 8],
    /// Mirroring de l'en-tete ($A000 : E27a3).
    mirroring: Mirroring,
}

impl Mmc3 {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        Mmc3 {
            prg_ram: vec![0; cart.prg_ram_size.max(PRG_RAM_MIN)],
            mirroring: cart.mirroring,
            prg_rom: cart.prg_rom,
            chr,
            bank_select: 0,
            regs: [0; 8],
        }
    }

    /// Nombre de banques PRG de 8 Ko (minimum 1).
    fn nb_prg(&self) -> usize {
        (self.prg_rom.len() / PRG_BANK).max(1)
    }

    /// Banque PRG de 8 Ko vue a `addr` ($8000-$FFFF).
    fn prg_bank(&self, addr: u16) -> usize {
        let n = self.nb_prg();
        let avant_derniere = n.saturating_sub(2);
        let r6 = usize::from(self.regs[6] & 0x3F);
        let r7 = usize::from(self.regs[7] & 0x3F);
        let mode1 = self.bank_select & 0x40 != 0;
        let b = match (addr >> 13) & 3 {
            0 => {
                if mode1 {
                    avant_derniere
                } else {
                    r6
                }
            }
            1 => r7,
            2 => {
                if mode1 {
                    r6
                } else {
                    avant_derniere
                }
            }
            _ => n - 1,
        };
        b % n
    }

    fn prg_read(&self, addr: u16) -> Option<u8> {
        if self.prg_rom.is_empty() {
            return None;
        }
        let off = self.prg_bank(addr) * PRG_BANK + usize::from(addr & 0x1FFF);
        self.prg_rom.get(off).copied()
    }

    fn ram_offset(&self, addr: u16) -> usize {
        usize::from(addr - 0x6000) % self.prg_ram.len()
    }

    /// Lecture sans effet de bord : meme valeur que `cpu_read`.
    fn read_at(&self, addr: u16) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF => Some(self.prg_ram[self.ram_offset(addr)]),
            0x8000..=0xFFFF => self.prg_read(addr),
            _ => None,
        }
    }

    /// Registres $8000-$FFFF : adresse paire ou impaire dans chaque plage de 8 Ko.
    fn write_register(&mut self, addr: u16, value: u8) {
        match addr & 0xE001 {
            0x8000 => self.bank_select = value,
            0x8001 => self.regs[usize::from(self.bank_select & 7)] = value,
            _ => {} // $A000-$A001 : E27a3 ; $C000-$E001 : IRQ (E28a1)
        }
    }
}

impl Mapper for Mmc3 {
    fn cpu_read(&mut self, addr: u16) -> Option<u8> {
        self.read_at(addr)
    }

    fn cpu_peek(&self, addr: u16) -> Option<u8> {
        self.read_at(addr) // aucune lecture n'a d'effet de bord
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let o = self.ram_offset(addr);
                self.prg_ram[o] = value;
            }
            0x8000..=0xFFFF => self.write_register(addr, value),
            _ => {}
        }
    }

    // CHR non banke pour l'instant (banques de 1 Ko : E27a2).
    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr.read(usize::from(addr))
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        self.chr.write(usize::from(addr), value);
    }

    fn ppu_peek(&self, addr: u16) -> u8 {
        self.chr.read(usize::from(addr))
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cartouche MMC3 : `prg` banques de 8 Ko et `chr` banques de 1 Ko (octet = numero de banque).
    fn cart(prg: usize, chr: usize) -> Cartridge {
        Cartridge {
            mapper_id: 4,
            submapper: 0,
            prg_rom: (0..prg * PRG_BANK).map(|i| (i / PRG_BANK) as u8).collect(),
            chr_rom: (0..chr * 1024).map(|i| (i / 1024) as u8).collect(),
            chr_ram_size: 0,
            prg_ram_size: 8 * 1024,
            mirroring: Mirroring::Vertical,
            has_battery: false,
            is_nes2: false,
        }
    }

    /// R`r` = `v` via $8000 (avec les bits de mode `mode` : 0x40 PRG, 0x80 CHR) puis $8001.
    fn set_reg(m: &mut Mmc3, mode: u8, r: u8, v: u8) {
        m.cpu_write(0x8000, mode | r);
        m.cpu_write(0x8001, v);
    }

    /// Lit les 4 fenetres PRG ($8000, $A000, $C000, $E000).
    fn prg4(m: &mut Mmc3) -> [Option<u8>; 4] {
        [0x8000, 0xA000, 0xC000, 0xE000].map(|a| m.cpu_read(a))
    }

    #[test]
    fn dernier_fixe() {
        let mut m = Mmc3::new(cart(16, 8));
        assert_eq!(m.cpu_read(0xE000), Some(15));
        assert_eq!(m.cpu_read(0xFFFF), Some(15));
        assert_eq!(m.cpu_read(0xC000), Some(14)); // mode 0 : avant-derniere
    }

    #[test]
    fn prg_mode0() {
        let mut m = Mmc3::new(cart(16, 8));
        set_reg(&mut m, 0, 6, 3);
        set_reg(&mut m, 0, 7, 4);
        assert_eq!(prg4(&mut m), [Some(3), Some(4), Some(14), Some(15)]);
    }

    #[test]
    fn prg_mode1() {
        let mut m = Mmc3::new(cart(16, 8));
        set_reg(&mut m, 0x40, 6, 3);
        set_reg(&mut m, 0x40, 7, 4);
        assert_eq!(prg4(&mut m), [Some(14), Some(4), Some(3), Some(15)]);
    }

    #[test]
    fn prg_modulo() {
        let mut m = Mmc3::new(cart(16, 8));
        set_reg(&mut m, 0, 6, 19); // 19 % 16 = 3
        assert_eq!(m.cpu_read(0x8000), Some(3));
    }
}
