//! MMC1 (mapper 1) : registre serie 5 bits, Control, banques PRG/CHR, PRG-RAM, SUROM.
// wiki: MMC1 ; wiki: INES_Mapper_001

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_BANK: usize = 16 * 1024;
const PRG_RAM_MIN: usize = 8 * 1024;

// ---------------------------------------------------------------- E26a1 : registre serie

/// Resultat d'une ecriture dans le registre serie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialEvent {
    /// Bit accumule, ou ecriture ignoree (cycle consecutif).
    Rien,
    /// Bit 7 = 1 : registre vide ; l'appelant fait `control |= 0x0C`.
    Reset,
    /// 5e ecriture : (registre 0..=3 = bits 13-14 de l'adresse, valeur 5 bits).
    Ecrit(u8, u8),
}

/// Registre a decalage du MMC1 (bit de poids faible en premier).
#[derive(Debug, Default, Clone, Copy)]
pub struct Serial {
    shift: u8,
    count: u8,
    /// Compteur de cycles CPU (incremente par `tick`, appele depuis `Mapper::cpu_cycle`).
    cycle: u64,
    /// Cycle de la derniere ecriture (None = aucune).
    last_write: Option<u64>,
}

impl Serial {
    /// Un cycle CPU (M2).
    pub fn tick(&mut self) {
        self.cycle += 1;
    }

    /// Ecriture CPU en $8000-$FFFF.
    pub fn write(&mut self, addr: u16, value: u8) -> SerialEvent {
        // Ecritures sur des cycles consecutifs (instructions RMW) : seule la 1re compte.
        let ignoree = matches!(self.last_write, Some(c) if self.cycle == c + 1);
        self.last_write = Some(self.cycle);
        if ignoree {
            return SerialEvent::Rien;
        }
        if value & 0x80 != 0 {
            self.shift = 0;
            self.count = 0;
            return SerialEvent::Reset;
        }
        self.shift |= (value & 1) << self.count;
        self.count += 1;
        if self.count < 5 {
            return SerialEvent::Rien;
        }
        let v = self.shift;
        self.shift = 0;
        self.count = 0;
        SerialEvent::Ecrit(((addr >> 13) & 3) as u8, v)
    }
}

// ---------------------------------------------------------------- E26a2 : Control, PRG, mirroring

/// Cartouche MMC1 (SxROM).
pub struct Mmc1 {
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr: ChrMemory,
    serial: Serial,
    /// Control `CPPMM` (mise sous tension : 0x0C = mode PRG 3).
    control: u8,
    chr0: u8,
    chr1: u8,
    prg: u8,
}

impl Mmc1 {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        Mmc1 {
            prg_ram: vec![0; cart.prg_ram_size.max(PRG_RAM_MIN)],
            prg_rom: cart.prg_rom,
            chr,
            serial: Serial::default(),
            control: 0x0C,
            chr0: 0,
            chr1: 0,
            prg: 0,
        }
    }

    /// Registres (control, chr0, chr1, prg), pour les tests et le debogage.
    pub fn registers(&self) -> (u8, u8, u8, u8) {
        (self.control, self.chr0, self.chr1, self.prg)
    }

    /// Nombre de banques PRG de 16 Ko (minimum 1).
    fn nb_prg_banks(&self) -> usize {
        (self.prg_rom.len() / PRG_BANK).max(1)
    }

    /// Banque de 16 Ko vue en $8000 (`haut` = false) ou en $C000 (`haut` = true).
    fn prg_bank(&self, haut: bool) -> usize {
        let bank = usize::from(self.prg & 0x0F);
        let last = self.nb_prg_banks() - 1;
        match (self.control >> 2) & 3 {
            0 | 1 => (bank & !1) | usize::from(haut), // 32 Ko
            2 => {
                if haut {
                    bank
                } else {
                    0
                }
            }
            _ => {
                if haut {
                    last
                } else {
                    bank
                }
            }
        }
    }

    fn prg_read(&self, addr: u16) -> Option<u8> {
        if self.prg_rom.is_empty() {
            return None;
        }
        let bank = self.prg_bank(addr >= 0xC000) % self.nb_prg_banks();
        self.prg_rom
            .get(bank * PRG_BANK + usize::from(addr & 0x3FFF))
            .copied()
    }

    /// Offset dans la PRG-RAM ($6000-$7FFF, miroir si plus courte que 8 Ko).
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

    /// Applique un evenement du registre serie.
    fn apply(&mut self, ev: SerialEvent) {
        match ev {
            SerialEvent::Rien => {}
            SerialEvent::Reset => self.control |= 0x0C,
            SerialEvent::Ecrit(0, v) => self.control = v,
            SerialEvent::Ecrit(1, v) => self.chr0 = v,
            SerialEvent::Ecrit(2, v) => self.chr1 = v,
            SerialEvent::Ecrit(_, v) => self.prg = v,
        }
    }
}

impl Mapper for Mmc1 {
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
            0x8000..=0xFFFF => {
                let ev = self.serial.write(addr, value);
                self.apply(ev);
            }
            _ => {}
        }
    }

    // CHR non banke pour l'instant (banques CHR : E26b1).
    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr.read(usize::from(addr))
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        self.chr.write(usize::from(addr), value);
    }

    fn ppu_peek(&self, addr: u16) -> u8 {
        self.chr.read(usize::from(addr))
    }

    /// Mirroring pilote par Control (bits 0-1), pas par l'en-tete.
    fn mirroring(&self) -> Mirroring {
        match self.control & 3 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            _ => Mirroring::Horizontal,
        }
    }

    fn cpu_cycle(&mut self) {
        self.serial.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 5 ecritures de `v5` (poids faible d'abord), 2 cycles avant chacune ; renvoie le dernier evenement.
    fn serie(s: &mut Serial, addr: u16, v5: u8) -> SerialEvent {
        let mut ev = SerialEvent::Rien;
        for i in 0..5 {
            s.tick();
            s.tick();
            ev = s.write(addr, (v5 >> i) & 1);
        }
        ev
    }

    #[test]
    fn serie_cinq_bits() {
        let mut s = Serial::default();
        assert_eq!(
            serie(&mut s, 0xE000, 0b10110),
            SerialEvent::Ecrit(3, 0b10110)
        );
        // Registre vide apres la 5e ecriture : la valeur suivante repart de zero.
        assert_eq!(
            serie(&mut s, 0xE000, 0b00001),
            SerialEvent::Ecrit(3, 0b00001)
        );
    }

    #[test]
    fn serie_registre_par_adresse() {
        let mut s = Serial::default();
        assert_eq!(serie(&mut s, 0x8000, 1), SerialEvent::Ecrit(0, 1));
        assert_eq!(serie(&mut s, 0xBFFF, 2), SerialEvent::Ecrit(1, 2));
        assert_eq!(serie(&mut s, 0xC123, 3), SerialEvent::Ecrit(2, 3));
        assert_eq!(serie(&mut s, 0xFFFF, 4), SerialEvent::Ecrit(3, 4));
    }

    #[test]
    fn serie_reset() {
        let mut s = Serial::default();
        s.tick();
        s.tick();
        assert_eq!(s.write(0x8000, 1), SerialEvent::Rien); // 1 bit en attente...
        s.tick();
        s.tick();
        assert_eq!(s.write(0x8000, 0x80), SerialEvent::Reset); // ... efface
        assert_eq!(
            serie(&mut s, 0xE000, 0b00010),
            SerialEvent::Ecrit(3, 0b00010)
        );
    }

    #[test]
    fn serie_consecutives() {
        let mut s = Serial::default();
        s.tick();
        s.tick();
        assert_eq!(s.write(0xE000, 1), SerialEvent::Rien);
        s.tick(); // cycle suivant : ecriture ignoree (meme un reset)
        assert_eq!(s.write(0xE000, 0x80), SerialEvent::Rien);
        let mut ev = SerialEvent::Rien;
        for _ in 0..4 {
            s.tick();
            s.tick();
            ev = s.write(0xE000, 0);
        }
        assert_eq!(ev, SerialEvent::Ecrit(3, 1));
    }

    // ---------- E26a2 ----------

    /// Cartouche MMC1 synthetique : `banks` banques PRG de 16 Ko (octet = numero de banque), CHR-RAM 8 Ko.
    fn cart(banks: usize) -> Cartridge {
        Cartridge {
            mapper_id: 1,
            submapper: 0,
            prg_rom: (0..banks * PRG_BANK)
                .map(|i| (i / PRG_BANK) as u8)
                .collect(),
            chr_rom: Vec::new(),
            chr_ram_size: 8 * 1024,
            prg_ram_size: 8 * 1024,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
            is_nes2: false,
        }
    }

    /// 5 ecritures serie de `v5` en `addr`, 2 cycles CPU avant chacune.
    fn write_serial(m: &mut Mmc1, addr: u16, v5: u8) {
        for i in 0..5 {
            m.cpu_cycle();
            m.cpu_cycle();
            m.cpu_write(addr, (v5 >> i) & 1);
        }
    }

    #[test]
    fn mise_sous_tension() {
        let mut m = Mmc1::new(cart(8));
        assert_eq!(m.registers().0, 0x0C);
        assert_eq!(m.cpu_read(0x8000), Some(0));
        assert_eq!(m.cpu_read(0xC000), Some(7)); // derniere banque
    }

    #[test]
    fn serie_prg() {
        let mut m = Mmc1::new(cart(8));
        write_serial(&mut m, 0xE000, 3);
        assert_eq!(m.cpu_read(0x8000), Some(3));
        assert_eq!(m.cpu_read(0xFFFF), Some(7));
    }

    #[test]
    fn reset_bit7() {
        let mut m = Mmc1::new(cart(8));
        write_serial(&mut m, 0x8000, 0x00); // Control = 0 (mode 32 Ko)
        m.cpu_cycle();
        m.cpu_cycle();
        m.cpu_write(0xE000, 1); // 1 bit en attente...
        m.cpu_cycle();
        m.cpu_cycle();
        m.cpu_write(0x8000, 0x80); // ... efface ; control |= 0x0C
        assert_eq!(m.registers().0, 0x0C);
        write_serial(&mut m, 0xE000, 3);
        assert_eq!(m.cpu_read(0x8000), Some(3));
        assert_eq!(m.cpu_read(0xC000), Some(7));
    }

    #[test]
    fn mode_prg_2() {
        let mut m = Mmc1::new(cart(8));
        write_serial(&mut m, 0x8000, 0b01000);
        write_serial(&mut m, 0xE000, 3);
        assert_eq!(m.cpu_read(0x8000), Some(0));
        assert_eq!(m.cpu_read(0xC000), Some(3));
    }

    #[test]
    fn mode_32k() {
        let mut m = Mmc1::new(cart(8));
        write_serial(&mut m, 0x8000, 0);
        write_serial(&mut m, 0xE000, 3); // bit 0 ignore : banques 2 et 3
        assert_eq!(m.cpu_read(0x8000), Some(2));
        assert_eq!(m.cpu_read(0xC000), Some(3));
    }

    #[test]
    fn mirroring() {
        let mut m = Mmc1::new(cart(8));
        let attendu = [
            Mirroring::SingleScreenLower,
            Mirroring::SingleScreenUpper,
            Mirroring::Vertical,
            Mirroring::Horizontal,
        ];
        for (mm, mir) in attendu.iter().enumerate() {
            write_serial(&mut m, 0x8000, 0x0C | mm as u8);
            assert_eq!(m.mirroring(), *mir);
        }
    }

    #[test]
    fn ecritures_consecutives() {
        let mut m = Mmc1::new(cart(8));
        m.cpu_cycle();
        m.cpu_cycle();
        m.cpu_write(0xE000, 1);
        m.cpu_cycle();
        m.cpu_write(0xE000, 1); // cycle consecutif : ignoree
        for _ in 0..4 {
            m.cpu_cycle();
            m.cpu_cycle();
            m.cpu_write(0xE000, 0);
        }
        assert_eq!(m.registers().3, 1); // et non 3
        assert_eq!(m.cpu_read(0x8000), Some(1));
    }
}
