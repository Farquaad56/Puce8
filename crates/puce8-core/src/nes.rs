//! Machine NES complete : CPU + Bus + PPU synchronises (E13b).

use crate::bus::Bus;
use crate::cartridge::{Cartridge, RomError};
use crate::cpu::Cpu;
use crate::cpu::CpuBus;

/// Points PPU par cycle CPU (NTSC).
pub const PPU_DOTS_PER_CPU: u32 = 3;
/// Points PPU joues AVANT le tick CPU (reglable en E17b/E36 : 1, 2 ou 3).
pub const PPU_DOTS_BEFORE_CPU: u32 = 3;

/// Machine NES complete (CPU + Bus + PPU).
pub struct Nes {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Nes {
    /// Construit une machine a partir d'un fichier ROM iNES.
    pub fn from_rom(bytes: &[u8]) -> Result<Nes, RomError> {
        let cart = Cartridge::from_bytes(bytes)?;
        Ok(Nes {
            cpu: Cpu::new(),
            bus: Bus::with_ppu(cart)?,
        })
    }

    /// Un cycle CPU : PPU_DOTS_BEFORE_CPU points PPU, le tick CPU, puis les points restants.
    pub fn tick(&mut self) {
        for dot in 0..PPU_DOTS_PER_CPU {
            if dot == PPU_DOTS_BEFORE_CPU {
                self.cpu.tick(&mut self.bus);
            }
            self.bus.ppu.tick(self.bus.mapper.as_mut());
        }
        if PPU_DOTS_BEFORE_CPU >= PPU_DOTS_PER_CPU {
            self.cpu.tick(&mut self.bus);
        }
        self.bus.mapper.cpu_cycle();
        self.bus.cpu_cycles += 1;
    }

    /// Execute une instruction complete et retourne les cycles consommes.
    pub fn step_instruction(&mut self) -> u32 {
        let start = self.bus.cpu_cycles;
        loop {
            self.tick();
            if self.cpu.at_instruction_boundary() {
                break;
            }
        }
        (self.bus.cpu_cycles - start) as u32
    }

    /// Execute jusqu'a la fin de l'image courante.
    pub fn run_frame(&mut self) {
        while !self.bus.ppu.take_frame_complete() {
            self.tick();
        }
    }

    /// Reinitialise le CPU (la PPU et le bus restent tels quels).
    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    /// Lecture directe en memoire (pour les tests).
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.peek(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM iNES minimale valide (16K PRG-ROM remplie de NOP).
    fn rom_minimale() -> Vec<u8> {
        let mut rom = vec![0x4E, 0x45, 0x53, 0x1A]; // en-tete iNES
        rom.extend_from_slice(&[0x01, 0x00]); // 1 bank PRG (16K), 0 banks CHR
        rom.extend_from_slice(&[0; 12]); // reste de l'en-tete
        rom.extend_from_slice(&[0xEA; 16384]); // PRG-ROM remplie de NOP
        rom
    }

    #[test]
    fn nes_tick_3_points() {
        let mut nes = Nes::from_rom(&rom_minimale()).unwrap();
        let start_cycles = nes.bus.cpu_cycles;
        nes.tick();
        assert_eq!(nes.bus.cpu_cycles - start_cycles, 1);
        assert_eq!(nes.bus.ppu.position(), (0, 3));
    }

    #[test]
    fn run_frame_cycles() {
        let mut nes = Nes::from_rom(&rom_minimale()).unwrap();
        // Premiere image partielle (mise sous tension -> VBlank).
        nes.run_frame();
        // Deux images completes.
        let start = nes.bus.cpu_cycles;
        nes.run_frame();
        nes.run_frame();
        let elapsed = nes.bus.cpu_cycles - start;
        assert!(
            (59560..=59562).contains(&elapsed),
            "attendu ~59561 cycles pour 2 images, obtenu {}",
            elapsed
        );
    }

    #[test]
    fn cpu_recoit_nmi() {
        let mut rom = rom_minimale();
        let prg = 16; // offset du PRG dans le fichier
        rom[prg..prg + 3].copy_from_slice(&[0x4C, 0x00, 0x80]); // $8000 : JMP $8000
        rom[prg + 0x10..prg + 0x14].copy_from_slice(&[0xEE, 0x00, 0x02, 0x40]); // $8010 : INC $0200 ; RTI
        rom[prg + 0x3FFA..prg + 0x4000].copy_from_slice(&[0x10, 0x80, 0x00, 0x80, 0x00, 0x80]); // NMI=$8010 RESET=$8000 IRQ=$8000
        let mut nes = Nes::from_rom(&rom).unwrap();
        nes.bus.write(0x2000, 0x80); // NMI activee
        for n in 1..=3u8 {
            nes.run_frame(); // s'arrete en (241, 1)
            for _ in 0..30 {
                nes.tick(); // fin d'instruction + sequence NMI + INC
            }
            assert_eq!(nes.peek(0x0200), n); // exactement une NMI par image
        }
    }
}
