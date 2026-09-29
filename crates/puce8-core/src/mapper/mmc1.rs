//! MMC1 (mapper 1) : registre serie 5 bits, Control, banques PRG/CHR, PRG-RAM, SUROM.
// wiki: MMC1 ; wiki: INES_Mapper_001

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_BANK: usize = 16 * 1024;
const PRG_RAM_MIN: usize = 8 * 1024;
const CHR_BANK: usize = 4 * 1024;

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
    has_battery: bool,
}

impl Mmc1 {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        Mmc1 {
            prg_ram: vec![0; cart.prg_ram_size.max(PRG_RAM_MIN)],
            has_battery: cart.has_battery,
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
        // SUROM (512 Ko) : le bit 4 de CHR0 choisit la moitie de 256 Ko (E26b3).
        let base = if self.prg_rom.len() > 256 * 1024 {
            usize::from(self.chr0 & 0x10)
        } else {
            0
        };
        let bank = usize::from(self.prg & 0x0F);
        let last = (self.nb_prg_banks() - 1).min(15);
        let b = match (self.control >> 2) & 3 {
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
        };
        base | b
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

    /// PRG-RAM active : bit 4 du registre PRG a 0 (E26b2).
    fn ram_enabled(&self) -> bool {
        self.prg & 0x10 == 0
    }

    /// Offset dans la PRG-RAM ($6000-$7FFF, miroir si plus courte que 8 Ko).
    fn ram_offset(&self, addr: u16) -> usize {
        usize::from(addr - 0x6000) % self.prg_ram.len()
    }

    /// Lecture sans effet de bord : meme valeur que `cpu_read`.
    fn read_at(&self, addr: u16) -> Option<u8> {
        match addr {
            0x6000..=0x7FFF if self.ram_enabled() => Some(self.prg_ram[self.ram_offset(addr)]),
            0x8000..=0xFFFF => self.prg_read(addr),
            _ => None,
        }
    }

    /// Offset CHR (E26b1) : 8 Ko (`CHR0 & 0x1E`) ou deux banques de 4 Ko (CHR0, CHR1).
    fn chr_offset(&self, addr: u16) -> usize {
        let a = usize::from(addr & 0x1FFF);
        if self.control & 0x10 == 0 {
            usize::from(self.chr0 & 0x1E) * CHR_BANK + a
        } else if a < 0x1000 {
            usize::from(self.chr0) * CHR_BANK + a
        } else {
            usize::from(self.chr1) * CHR_BANK + (a & 0x0FFF)
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
            0x6000..=0x7FFF if self.ram_enabled() => {
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

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr.read(self.chr_offset(addr))
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        let o = self.chr_offset(addr);
        self.chr.write(o, value);
    }

    fn ppu_peek(&self, addr: u16) -> u8 {
        self.chr.read(self.chr_offset(addr))
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
            tv_system: crate::region::TvSystem::Inconnu,
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

    // ---------- E26b1 ----------

    /// Comme `cart(8)`, avec une CHR-ROM de `n` banques de 4 Ko (octet = numero de banque).
    fn cart_chr(n: usize) -> Cartridge {
        let mut c = cart(8);
        c.chr_rom = (0..n * CHR_BANK).map(|i| (i / CHR_BANK) as u8).collect();
        c.chr_ram_size = 0;
        c
    }

    #[test]
    fn chr_4k() {
        let mut m = Mmc1::new(cart_chr(8));
        write_serial(&mut m, 0x8000, 0x1C); // CHR 4 Ko, mode PRG 3
        write_serial(&mut m, 0xA000, 5);
        write_serial(&mut m, 0xC000, 2);
        assert_eq!(m.ppu_read(0x0000), 5);
        assert_eq!(m.ppu_read(0x0FFF), 5);
        assert_eq!(m.ppu_read(0x1000), 2);
        assert_eq!(m.ppu_peek(0x1FFF), 2);
    }

    #[test]
    fn chr_8k() {
        let mut m = Mmc1::new(cart_chr(8));
        write_serial(&mut m, 0x8000, 0x0C); // CHR 8 Ko
        write_serial(&mut m, 0xA000, 5); // bit 0 ignore : banques 4 et 5
        write_serial(&mut m, 0xC000, 2); // ignore en mode 8 Ko
        assert_eq!(m.ppu_read(0x0000), 4);
        assert_eq!(m.ppu_read(0x1000), 5);
    }

    // ---------- E26b2 ----------

    #[test]
    fn prg_ram_desactivee() {
        let mut m = Mmc1::new(cart(8));
        m.cpu_write(0x6000, 0x42);
        assert_eq!(m.cpu_read(0x6000), Some(0x42));
        write_serial(&mut m, 0xE000, 0x10); // PRG bit 4 = 1 : RAM desactivee
        assert_eq!(m.cpu_read(0x6000), None);
        assert_eq!(m.cpu_peek(0x6000), None);
        m.cpu_write(0x6000, 0x99); // ignoree
        write_serial(&mut m, 0xE000, 0x00);
        assert_eq!(m.cpu_read(0x6000), Some(0x42));
    }

    #[test]
    fn batterie() {
        let mut c = cart(8);
        c.has_battery = true;
        let mut m = Mmc1::new(c);
        m.load_battery_ram(&[1, 2, 3]);
        assert_eq!(m.cpu_read(0x6001), Some(2));
        assert_eq!(m.battery_ram().map(|r| r.len()), Some(8 * 1024));
        assert!(Mmc1::new(cart(8)).battery_ram().is_none());
    }

    // ---------- E26b3 ----------

    #[test]
    fn surom() {
        let mut m = Mmc1::new(cart(32)); // 512 Ko
        assert_eq!(m.cpu_read(0xC000), Some(15)); // moitie basse : derniere banque = 15
        write_serial(&mut m, 0xA000, 0x10); // CHR0 bit 4 = 1 : moitie haute
        assert_eq!(m.cpu_read(0xC000), Some(31));
        assert_eq!(m.cpu_read(0x8000), Some(16));
        write_serial(&mut m, 0xE000, 3);
        assert_eq!(m.cpu_read(0x8000), Some(19));
    }
}
