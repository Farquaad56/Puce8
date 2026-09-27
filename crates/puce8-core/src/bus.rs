//! Bus CPU : RAM 2 Ko, miroirs, open bus, stubs PPU/APU (E03b).
// wiki: Open_bus ; wiki: NES_memory_map

use crate::apu::Apu;
use crate::cartridge::{Cartridge, RomError};
use crate::controller::Controller;
use crate::cpu::CpuBus;
use crate::dma::{OamDma, GET_PARITY};
use crate::mapper::{create_mapper, Mapper, Mirroring};
use crate::ppu::Ppu;

/// Bus CPU vu par le 6502 : RAM + cartouche + open bus.
pub struct Bus {
    /// $0000-$1FFF (miroir sur les 11 bits bas), 2 Ko.
    pub ram: [u8; 0x800],
    /// Cartouche : PRG-ROM, PRG-RAM, CHR... ($4020-$FFFF).
    pub mapper: Box<dyn Mapper>,
    /// PPU branchee sur $2000-$3FFF.
    pub ppu: Ppu,
    /// Dernier octet lu avec succes ou ecrit (open bus).
    pub open_bus: u8,
    /// Compteur de cycles CPU (cadence plus tard par Nes::tick).
    pub cpu_cycles: u64,
    /// DMA OAM en cours (E16b) : vole des cycles au CPU apres une ecriture $4014.
    pub dma: OamDma,
    /// Manettes standard $4016/$4017 (E24a2).
    pub controller: Controller,
    /// APU $4000-$4017 (E30c1).
    pub apu: Apu,
}

impl Bus {
    pub fn new(mapper: Box<dyn Mapper>) -> Self {
        Bus {
            ram: [0u8; 0x800],
            mapper,
            ppu: Ppu::new(),
            open_bus: 0,
            cpu_cycles: 0,
            dma: OamDma::Idle,
            controller: Controller::new(),
            apu: Apu::new(),
        }
    }

    /// Vrai si la DMA prend ce cycle a la place du CPU.
    pub fn dma_halts_cpu(&self, cpu_next_is_read: bool) -> bool {
        match self.dma {
            OamDma::Idle => false,
            OamDma::Requested(_) => cpu_next_is_read, // l'arret attend une lecture du CPU
            _ => true,
        }
    }

    /// Un cycle de DMA. `c = self.cpu_cycles` est le numero du cycle en cours
    /// (Nes::tick l'incremente APRES). Transitions EXACTES :
    pub fn dma_tick(&mut self) {
        let c = self.cpu_cycles;
        let is_get = |n: u64| n % 2 == GET_PARITY;
        self.dma = match self.dma {
            OamDma::Idle => OamDma::Idle,
            // Cycle d'arret. SIMPLIFICATION: aucun acces bus (revu en E36).
            OamDma::Requested(p) | OamDma::Halt(p) => {
                if is_get(c + 1) {
                    OamDma::Get { page: p, i: 0 }
                } else {
                    OamDma::Align(p)
                }
            }
            // Cycle d'alignement. SIMPLIFICATION: aucun acces bus (revu en E36).
            OamDma::Align(p) => OamDma::Get { page: p, i: 0 },
            OamDma::Get { page, i } => {
                let v = self.read(u16::from(page) << 8 | u16::from(i));
                OamDma::Put { page, i, v }
            }
            OamDma::Put { page, i, v } => {
                // $2004 : oam[oam_addr] = v ; oam_addr += 1
                self.ppu.cpu_write_register(4, v, self.mapper.as_mut());
                if i == 0xFF {
                    OamDma::Idle
                } else {
                    OamDma::Get { page, i: i + 1 }
                }
            }
        };
    }

    /// Construit un bus a partir d'une cartouche (E13b).
    pub fn with_ppu(cart: Cartridge) -> Result<Self, RomError> {
        let mapper = create_mapper(cart)?;
        Ok(Bus::new(mapper))
    }

    /// Helper de test : NROM synthetique (mapper 0) dont la PRG-ROM est `prg`.
    pub fn for_test_with_prg(prg: &[u8]) -> Bus {
        let cart = Cartridge {
            mapper_id: 0,
            submapper: 0,
            prg_rom: prg.to_vec(),
            chr_rom: Vec::new(),
            chr_ram_size: 8 * 1024,
            prg_ram_size: 8 * 1024,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
            is_nes2: false,
        };
        let mapper = create_mapper(cart).expect("mapper 0 (NROM) est toujours supporte");
        Bus::new(mapper)
    }
}

impl CpuBus for Bus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            // $0000-$1FFF : RAM 2 Ko, miroir sur les 11 bits bas.
            0x0000..=0x1FFF => {
                let value = self.ram[usize::from(addr & 0x07FF)];
                self.open_bus = value; // l'open bus prend chaque octet lu avec succes
                value
            }
            // $2000-$3FFF : PPU (miroir sur les 11 bits bas).
            0x2000..=0x3FFF => {
                let value = self.ppu.cpu_read_register(addr & 7, self.mapper.as_mut());
                self.open_bus = value;
                value
            }
            // E30c1 : etat APU ; bit 5 = open bus ; cette lecture ne modifie pas l'open bus.
            0x4015 => (self.open_bus & 0x20) | self.apu.read_status(),
            // E24a2 : manettes ; bits 5-7 = open bus, puis l'octet lu devient l'open bus.
            0x4016 | 0x4017 => {
                let value = self.controller.read(usize::from(addr & 1), self.open_bus);
                self.open_bus = value;
                value
            }
            // $4000-$401F : APU (stubs).
            0x4000..=0x401F => self.open_bus,
            // $4020-$FFFF : cartouche.
            _ => match self.mapper.cpu_read(addr) {
                Some(value) => {
                    self.open_bus = value;
                    value
                }
                None => self.open_bus,
            },
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram[usize::from(addr & 0x07FF)] = value;
                self.open_bus = value; // l'open bus prend chaque octet ecrit
            }
            // $2000-$3FFF : PPU (miroir sur les 11 bits bas).
            0x2000..=0x3FFF => {
                self.ppu
                    .cpu_write_register(addr & 7, value, self.mapper.as_mut());
                self.open_bus = value;
            }
            // $4014 : demande de DMA OAM (E16b) ; la derniere ecriture gagne.
            0x4014 => {
                self.dma.request_oam(value);
                self.open_bus = value;
            }
            // E24a2 : strobe des deux manettes ($4017 en ecriture = APU, plus tard).
            0x4016 => {
                self.controller.write(value);
                self.open_bus = value;
            }
            // E30c1 : registres APU ($4014 = DMA et $4016 = manettes sont traites au-dessus).
            0x4000..=0x4013 | 0x4015 | 0x4017 => {
                self.apu.write_register(addr, value);
                self.open_bus = value;
            }
            // $4000-$401F : APU (stubs).
            0x4000..=0x401F => {
                self.open_bus = value;
            }
            _ => {
                self.mapper.cpu_write(addr, value);
                self.open_bus = value;
            }
        }
    }

    fn peek(&self, addr: u16) -> u8 {
        // Meme decodage que `read`, sans toucher l'open bus.
        match addr {
            0x0000..=0x1FFF => self.ram[usize::from(addr & 0x07FF)],
            // $2000-$3FFF : PPU (miroir sur les 11 bits bas).
            0x2000..=0x3FFF => self.ppu.cpu_peek_register(addr & 7),
            0x4015 => (self.open_bus & 0x20) | self.apu.peek_status(),
            0x4016 | 0x4017 => self.controller.peek(usize::from(addr & 1), self.open_bus),
            // $4000-$401F : APU (stubs).
            0x4000..=0x401F => self.open_bus,
            _ => self.mapper.cpu_peek(addr).unwrap_or(self.open_bus),
        }
    }

    fn nmi_line(&self) -> bool {
        self.ppu.nmi_line()
    }

    fn irq_line(&self) -> bool {
        self.mapper.irq_pending() || self.apu.irq_line() // E28b1 + E30c1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_miroirs() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x0001, 0x55);
        assert_eq!(bus.read(0x0801), 0x55);
        assert_eq!(bus.read(0x1001), 0x55);
        assert_eq!(bus.read(0x1801), 0x55);
    }

    #[test]
    fn ram_miroir_ecriture() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x1FFF, 0xAB);
        assert_eq!(bus.ram[0x7FF], 0xAB);
    }

    #[test]
    fn lecture_prg() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        assert_eq!(bus.read(0x8000), 0xA9);
    }

    #[test]
    fn open_bus_stub_io() {
        let mut bus = Bus::for_test_with_prg(&[0x33]);
        assert_eq!(bus.read(0x8000), 0x33); // PRG[0] -> open bus
        assert_eq!(bus.read(0x4018), 0x33); // stub IO -> open bus
    }

    #[test]
    fn open_bus_apres_ecriture() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x0000, 0x77);
        assert_eq!(bus.read(0x5000), 0x77); // rien de branche -> open bus
    }

    #[test]
    fn peek_sans_effet() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        assert_eq!(bus.read(0x8000), 0xA9); // open_bus = 0xA9
        for i in 0..10u16 {
            let _ = bus.peek(0x8000 + i);
        }
        assert_eq!(bus.open_bus, 0xA9);
    }

    /// Fait courir la DMA jusqu'a son retour a l'etat Idle ; renvoie le nombre de cycles.
    fn run_dma(bus: &mut Bus) -> u64 {
        let start = bus.cpu_cycles;
        while bus.dma_halts_cpu(true) {
            bus.dma_tick();
            bus.cpu_cycles += 1;
        }
        bus.cpu_cycles - start
    }

    #[test]
    fn dma_copie() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        for i in 0..=255u8 {
            bus.ram[0x200 + usize::from(i)] = i; // page $02 : ram[$0200+i] = i
        }
        bus.write(0x4014, 0x02);
        run_dma(&mut bus);
        for i in 0..=255usize {
            assert_eq!(bus.ppu.oam[i], i as u8);
        }
    }

    #[test]
    fn dma_oam_addr() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x2003, 0x10); // oam_addr = $10
        for i in 0..=255u8 {
            bus.ram[0x200 + usize::from(i)] = i;
        }
        bus.write(0x4014, 0x02);
        run_dma(&mut bus);
        for i in 0..=255usize {
            assert_eq!(bus.ppu.oam[(0x10 + i) & 0xFF], i as u8);
        }
        assert_eq!(bus.ppu.regs.oam_addr, 0x10); // 256 increments = tour complet
    }

    #[test]
    fn dma_derniere_page() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x4014, 0x02);
        bus.write(0x4014, 0x03); // la derniere ecriture gagne
        assert_eq!(bus.dma, OamDma::Requested(0x03));
    }

    #[test]
    fn dma_attend_lecture_bus() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x4014, 2);
        assert!(!bus.dma_halts_cpu(false)); // pas de lecture CPU -> la DMA ne prend pas le cycle
        assert!(bus.dma_halts_cpu(true)); // lecture CPU -> la DMA prend le cycle
    }

    #[test]
    fn dma_parite() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        for i in 0..=255u8 {
            bus.ram[0x200 + usize::from(i)] = i;
        }
        bus.cpu_cycles = 11; // cycle impair (put) : pas d'alignement -> 513
        bus.write(0x4014, 0x02);
        assert_eq!(run_dma(&mut bus), 513);

        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        for i in 0..=255u8 {
            bus.ram[0x200 + usize::from(i)] = i;
        }
        bus.cpu_cycles = 10; // cycle pair (get) : alignement -> 514
        bus.write(0x4014, 0x02);
        assert_eq!(run_dma(&mut bus), 514);
    }

    #[test]
    fn bus_apu_4015_efface() {
        let mut bus = Bus::for_test_with_prg(&[0xEA]);
        bus.apu.frame.irq = true;
        assert!(bus.irq_line());
        assert_eq!(bus.peek(0x4015) & 0x40, 0x40); // peek : sans effet
        assert_eq!(bus.read(0x4015) & 0x40, 0x40);
        assert_eq!(bus.read(0x4015) & 0x40, 0); // la lecture efface F
        assert!(!bus.irq_line());
    }

    #[test]
    fn bus_apu_bits_longueur() {
        let mut bus = Bus::for_test_with_prg(&[0xEA]);
        bus.write(0x4015, 0x0F);
        bus.write(0x4003, 0x08); // pulse 1 : longueur 254
        bus.write(0x400F, 0x08); // bruit
        assert_eq!(bus.read(0x4015) & 0x0F, 0x09);
        bus.write(0x4015, 0x00);
        assert_eq!(bus.read(0x4015) & 0x0F, 0);
    }
}
