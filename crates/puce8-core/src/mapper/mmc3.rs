//! MMC3 (mapper 4) : banques PRG 8 Ko / CHR 1 Ko, mirroring, PRG-RAM (E27a) ; IRQ de ligne (E28a).
// wiki: MMC3 ; wiki: INES_Mapper_004

use crate::cartridge::Cartridge;
use crate::mapper::{ChrMemory, Mapper, Mirroring};

const PRG_BANK: usize = 8 * 1024;
const CHR_BANK: usize = 1024;
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
    /// Mirroring courant ($A000) ; four-screen (en-tete) : fixe.
    mirroring: Mirroring,
    /// $A001 : bit 7 = PRG-RAM active, bit 6 = protegee en ecriture (E27a3).
    ram_protect: u8,
    has_battery: bool,
    /// E28a1 : compteur de lignes et IRQ.
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq: bool,
}

impl Mmc3 {
    pub fn new(cart: Cartridge) -> Self {
        let chr = ChrMemory::from_cart(&cart);
        Mmc3 {
            prg_ram: vec![0; cart.prg_ram_size.max(PRG_RAM_MIN)],
            mirroring: cart.mirroring,
            has_battery: cart.has_battery,
            prg_rom: cart.prg_rom,
            chr,
            bank_select: 0,
            regs: [0; 8],
            ram_protect: 0x80, // SIMPLIFICATION: PRG-RAM active et inscriptible a la mise sous tension
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq: false,
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

    /// Banque CHR de 1 Ko vue a `addr` ($0000-$1FFF) (E27a2).
    fn chr_bank(&self, addr: u16) -> usize {
        let slot = usize::from((addr >> 10) & 7);
        // Inversion CHR (bit 7 de $8000) : les moities $0000 et $1000 sont echangees.
        let s = if self.bank_select & 0x80 != 0 {
            slot ^ 4
        } else {
            slot
        };
        let r = &self.regs;
        usize::from(match s {
            0 => r[0] & 0xFE,
            1 => r[0] | 1,
            2 => r[1] & 0xFE,
            3 => r[1] | 1,
            4 => r[2],
            5 => r[3],
            6 => r[4],
            _ => r[5],
        })
    }

    fn chr_offset(&self, addr: u16) -> usize {
        self.chr_bank(addr) * CHR_BANK + usize::from(addr & 0x03FF)
    }

    /// PRG-RAM lisible : bit 7 de $A001 (E27a3).
    fn ram_enabled(&self) -> bool {
        self.ram_protect & 0x80 != 0
    }

    /// PRG-RAM inscriptible : active ET non protegee (bit 6 de $A001 a 0).
    fn ram_writable(&self) -> bool {
        self.ram_protect & 0xC0 == 0x80
    }

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

    /// Registres $8000-$FFFF : adresse paire ou impaire dans chaque plage de 8 Ko.
    fn write_register(&mut self, addr: u16, value: u8) {
        match addr & 0xE001 {
            0x8000 => self.bank_select = value,
            0x8001 => self.regs[usize::from(self.bank_select & 7)] = value,
            0xA000 => {
                if self.mirroring != Mirroring::FourScreen {
                    self.mirroring = if value & 1 == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                }
            }
            0xA001 => self.ram_protect = value,
            0xC000 => self.irq_latch = value,
            0xC001 => {
                self.irq_counter = 0;
                self.irq_reload = true;
            }
            0xE000 => {
                self.irq_enabled = false;
                self.irq = false; // acquittement
            }
            _ => self.irq_enabled = true, // $E001
        }
    }

    /// Un clock du compteur de lignes (E28a1) ; appele sur un front montant filtre de A12 (E28a2).
    pub fn clock_counter(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter = self.irq_counter.wrapping_sub(1);
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq = true;
        }
    }

    /// Valeur du compteur de lignes (tests et debogage).
    pub fn counter(&self) -> u8 {
        self.irq_counter
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
            0x6000..=0x7FFF if self.ram_writable() => {
                let o = self.ram_offset(addr);
                self.prg_ram[o] = value;
            }
            0x8000..=0xFFFF => self.write_register(addr, value),
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

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn irq_pending(&self) -> bool {
        self.irq
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

    // ---------- E27a2 ----------

    #[test]
    fn chr_mode0() {
        let mut m = Mmc3::new(cart(16, 32));
        set_reg(&mut m, 0, 0, 5); // R0 : 2 Ko, bit 0 ignore -> 4 et 5
        set_reg(&mut m, 0, 2, 9);
        assert_eq!(m.ppu_read(0x0000), 4);
        assert_eq!(m.ppu_read(0x0400), 5);
        assert_eq!(m.ppu_read(0x1000), 9);
        assert_eq!(m.ppu_peek(0x13FF), 9);
    }

    #[test]
    fn chr_mode1() {
        let mut m = Mmc3::new(cart(16, 32));
        set_reg(&mut m, 0x80, 2, 7); // inversion : R2 en $0000
        set_reg(&mut m, 0x80, 0, 5); // R0 en $1000-$17FF
        assert_eq!(m.ppu_read(0x0000), 7);
        assert_eq!(m.ppu_read(0x1000), 4);
        assert_eq!(m.ppu_read(0x1400), 5);
    }

    // ---------- E27a3 ----------

    #[test]
    fn mirroring() {
        let mut m = Mmc3::new(cart(16, 8));
        assert_eq!(m.mirroring(), Mirroring::Vertical); // en-tete
        m.cpu_write(0xA000, 1);
        assert_eq!(m.mirroring(), Mirroring::Horizontal);
        m.cpu_write(0xBFFE, 0); // miroir de $A000 (adresse paire)
        assert_eq!(m.mirroring(), Mirroring::Vertical);
        let mut c = cart(16, 8);
        c.mirroring = Mirroring::FourScreen;
        let mut m = Mmc3::new(c);
        m.cpu_write(0xA000, 1); // ignore en four-screen
        assert_eq!(m.mirroring(), Mirroring::FourScreen);
    }

    #[test]
    fn prg_ram_protect() {
        let mut m = Mmc3::new(cart(16, 8));
        m.cpu_write(0x6000, 0x11); // mise sous tension : active, inscriptible
        assert_eq!(m.cpu_read(0x6000), Some(0x11));
        m.cpu_write(0xA001, 0xC0); // protegee en ecriture
        m.cpu_write(0x6000, 0x22);
        assert_eq!(m.cpu_read(0x6000), Some(0x11));
        m.cpu_write(0xA001, 0x00); // desactivee : open bus
        assert_eq!(m.cpu_read(0x6000), None);
        m.cpu_write(0xA001, 0x80);
        m.cpu_write(0x6000, 0x33);
        assert_eq!(m.cpu_read(0x6000), Some(0x33));
    }

    #[test]
    fn batterie() {
        let mut c = cart(16, 8);
        c.has_battery = true;
        assert_eq!(Mmc3::new(c).battery_ram().map(|r| r.len()), Some(8 * 1024));
        assert!(Mmc3::new(cart(16, 8)).battery_ram().is_none());
    }

    // ---------- E28a1 ----------

    #[test]
    fn reload_c001() {
        let mut m = Mmc3::new(cart(16, 8));
        m.cpu_write(0xC000, 5); // latch
        m.cpu_write(0xC001, 0); // rechargement au prochain clock
        m.clock_counter();
        assert_eq!(m.counter(), 5);
        m.clock_counter();
        assert_eq!(m.counter(), 4);
        m.cpu_write(0xC001, 0);
        m.clock_counter();
        assert_eq!(m.counter(), 5);
    }

    #[test]
    fn decompte() {
        let mut m = Mmc3::new(cart(16, 8));
        m.cpu_write(0xC000, 2);
        m.cpu_write(0xC001, 0);
        m.cpu_write(0xE001, 0); // IRQ activee
        m.clock_counter(); // 2
        m.clock_counter(); // 1
        assert!(!m.irq_pending());
        m.clock_counter(); // 0 -> IRQ
        assert_eq!(m.counter(), 0);
        assert!(m.irq_pending());
    }

    #[test]
    fn latch_zero() {
        let mut m = Mmc3::new(cart(16, 8));
        m.cpu_write(0xC000, 0);
        m.cpu_write(0xE001, 0);
        m.clock_counter();
        assert!(m.irq_pending()); // IRQ a chaque clock
        m.cpu_write(0xE000, 0); // acquittement (et desactivation)
        m.cpu_write(0xE001, 0);
        assert!(!m.irq_pending());
        m.clock_counter();
        assert!(m.irq_pending());
    }

    #[test]
    fn e000_acquitte() {
        let mut m = Mmc3::new(cart(16, 8));
        m.cpu_write(0xC000, 1);
        m.cpu_write(0xE001, 0);
        m.clock_counter(); // 1
        m.clock_counter(); // 0 -> IRQ
        assert!(m.irq_pending());
        m.cpu_write(0xE000, 0);
        assert!(!m.irq_pending());
        m.clock_counter(); // recharge 1
        m.clock_counter(); // 0, mais IRQ desactivee
        assert!(!m.irq_pending());
    }

    #[test]
    fn desactivee() {
        let mut m = Mmc3::new(cart(16, 8));
        m.cpu_write(0xC000, 1);
        for _ in 0..4 {
            m.clock_counter();
        }
        assert!(!m.irq_pending()); // jamais activee ($E001 non ecrit)
    }
}
