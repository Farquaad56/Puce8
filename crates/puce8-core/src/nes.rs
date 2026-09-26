//! Machine NES complete : CPU + Bus + PPU synchronises (E13b).

use crate::bus::Bus;
use crate::cartridge::{Cartridge, RomError};
use crate::cpu::Cpu;
use crate::cpu::CpuBus;
use crate::ppu::Ppu;

/// La PPU tourne a 3x la frequence du CPU : 3 points PPU par cycle CPU.
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

    /// Un cycle CPU : 3 points PPU avant, le tick CPU, puis les points restants.
    pub fn tick(&mut self) {
        for _ in 0..PPU_DOTS_BEFORE_CPU {
            self.bus.ppu.tick(self.bus.mapper.as_mut());
        }
        self.cpu.tick(&mut self.bus);
        for _ in PPU_DOTS_BEFORE_CPU..3 {
            self.bus.ppu.tick(self.bus.mapper.as_mut());
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
    }

    #[test]
    fn run_frame_cycles() {
        let mut nes = Nes::from_rom(&rom_minimale()).unwrap();
        // Warm-up : passer le premier frame partiel (depuis la mise sous tension jusqu'a VBlank).
        nes.run_frame();

        // Mesurer deux frames completes.
        let start = nes.bus.cpu_cycles;
        nes.run_frame();
        nes.run_frame();
        let elapsed = nes.bus.cpu_cycles - start;
        assert!(
            elapsed >= 59560 && elapsed <= 59562,
            "attendu ~59561 cycles pour 2 images, obtenu {}",
            elapsed
        );
    }

    #[test]
    fn cpu_recoit_nmi() {
        // ROM synthetique : boucle infinie + vecteur NMI qui incremente $0200.
        let mut rom = rom_minimale();
        let prg_offset = 16;

        // Programme principal a $8000 (offset 0) : JMP * (boucle infinie).
        rom[prg_offset + 0] = 0x4C;
        rom[prg_offset + 1] = 0xFF;
        rom[prg_offset + 2] = 0xFF;

        // Handler NMI a $8003 (offset 3) : INC $0200 puis RTI.
        rom[prg_offset + 3] = 0xE6; // INC $0200
        rom[prg_offset + 4] = 0x00;
        rom[prg_offset + 5] = 0x02;
        rom[prg_offset + 6] = 0x40; // RTI

        // Vecteur RESET a $FFFC : pointe vers $8000 (programme principal).
        rom[prg_offset + 0x3FFC] = 0x00; // low byte of $8000
        rom[prg_offset + 0x3FFD] = 0x80; // high byte of $8000

        // Vecteur NMI a $FFFA : pointe vers $8003 (handler).
        rom[prg_offset + 0x3FFA] = 0x03; // low byte of $8003
        rom[prg_offset + 0x3FFB] = 0x80; // high byte of $8003

        let mut nes = Nes::from_rom(&rom).unwrap();

        // Activer le NMI via $2000 bit 7 (a travers le bus, pas directement sur la PPU).
        nes.bus.write(0x2000, 0x80);

        // Executer une image complete : le CPU doit recevoir un NMI et incrementer $0200.
        nes.run_frame();

        // Verifier que l'interruption a ete echantillonnee (need_nmi devrait etre true).
        assert!(nes.cpu.need_nmi, "need_nmi n'est pas pose apres run_frame");

        // Donner au CPU quelques cycles pour traiter l'interruption a la prochaine frontiere d'instruction.
        for _ in 0..10 {
            nes.tick();
        }

        assert_eq!(nes.peek(0x0200), 1, "le NMI n'a pas ete traite");
    }
}
