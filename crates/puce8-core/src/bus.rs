//! Bus CPU : RAM 2 Ko, miroirs, open bus, stubs PPU/APU (E03b).
// wiki: Open_bus ; wiki: NES_memory_map

use crate::cartridge::Cartridge;
use crate::cpu::CpuBus;
use crate::mapper::{create_mapper, Mapper, Mirroring};

/// Bus CPU vu par le 6502 : RAM + cartouche + open bus.
pub struct Bus {
    /// $0000-$1FFF (miroir sur les 11 bits bas), 2 Ko.
    pub ram: [u8; 0x800],
    /// Cartouche : PRG-ROM, PRG-RAM, CHR... ($4020-$FFFF).
    pub mapper: Box<dyn Mapper>,
    /// Dernier octet lu avec succès ou écrit (open bus).
    pub open_bus: u8,
    /// Compteur de cycles CPU (cadencé plus tard par Nes::tick).
    pub cpu_cycles: u64,
}

impl Bus {
    pub fn new(mapper: Box<dyn Mapper>) -> Self {
        Bus {
            ram: [0u8; 0x800],
            mapper,
            open_bus: 0,
            cpu_cycles: 0,
        }
    }

    /// Helper de test : NROM synthétique (mapper 0) dont la PRG-ROM est `prg`.
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
        let mapper = create_mapper(cart).expect("mapper 0 (NROM) est toujours supporté");
        Bus::new(mapper)
    }
}

impl CpuBus for Bus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            // $0000-$1FFF : RAM 2 Ko, miroir sur les 11 bits bas.
            0x0000..=0x1FFF => {
                let value = self.ram[usize::from(addr & 0x07FF)];
                self.open_bus = value; // l'open bus prend chaque octet lu avec succès
                value
            }
            // $2000-$401F : rien de branché → open bus.
            // E13: PPU ($2000-$2007) ; E30: APU ($4015-$4017).
            0x2000..=0x401F => self.open_bus,
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
                self.open_bus = value; // l'open bus prend chaque octet écrit
            }
            // E13: PPU ; E30: APU — écritures ignorées pour l'instant.
            0x2000..=0x401F => {
                self.open_bus = value;
            }
            _ => {
                self.mapper.cpu_write(addr, value);
                self.open_bus = value;
            }
        }
    }

    fn peek(&self, addr: u16) -> u8 {
        // Même décodage que `read`, sans toucher l'open bus.
        match addr {
            0x0000..=0x1FFF => self.ram[usize::from(addr & 0x07FF)],
            // E13: PPU ; E30: APU — stubs, aucune valeur branchée.
            0x2000..=0x401F => self.open_bus,
            _ => self.mapper.cpu_peek(addr).unwrap_or(self.open_bus),
        }
    }

    fn nmi_line(&self) -> bool {
        false // E13: la PPU pilotera le NMI.
    }

    fn irq_line(&self) -> bool {
        false // E28 (MMC3) / E30 (APU) : plus tard.
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
        assert_eq!(bus.read(0x8000), 0x33); // PRG[0] → open bus
        assert_eq!(bus.read(0x4018), 0x33); // stub IO → open bus
    }

    #[test]
    fn open_bus_apres_ecriture() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        bus.write(0x0000, 0x77);
        assert_eq!(bus.read(0x5000), 0x77); // rien de branché → open bus
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
}
