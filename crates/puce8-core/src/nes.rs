//! Machine NES complete : CPU + Bus + PPU synchronises (E13b).

use crate::bus::Bus;
use crate::cartridge::{Cartridge, RomError};
use crate::cpu::Cpu;
use crate::cpu::CpuBus;
use crate::dma::OamDma;

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

    /// Un cycle CPU : soit la DMA le vole, soit le CPU l'execute.
    fn cpu_or_dma_tick(&mut self) {
        if self.bus.dma_halts_cpu(self.cpu.next_access_is_read()) {
            self.bus.dma_tick();
        } else {
            self.cpu.tick(&mut self.bus);
        }
    }

    /// Un cycle CPU : PPU_DOTS_BEFORE_CPU points PPU, le tick CPU (ou DMA), puis les points restants.
    pub fn tick(&mut self) {
        for dot in 0..PPU_DOTS_PER_CPU {
            if dot == PPU_DOTS_BEFORE_CPU {
                self.cpu_or_dma_tick();
            }
            self.bus.ppu.tick(self.bus.mapper.as_mut());
        }
        if PPU_DOTS_BEFORE_CPU >= PPU_DOTS_PER_CPU {
            self.cpu_or_dma_tick();
        }
        self.bus.mapper.cpu_cycle();
        self.bus.apu.tick(); // E30c1
        self.bus.poll_dmc_dma(); // E33b2
        self.bus.cpu_cycles += 1;
    }

    /// Execute une instruction complete et retourne les cycles consommes.
    /// Continue tant que la DMA OAM n'est pas finie (elle vole des cycles au CPU).
    pub fn step_instruction(&mut self) -> u32 {
        let start = self.bus.cpu_cycles;
        loop {
            self.tick();
            if self.cpu.at_instruction_boundary() && self.bus.dma == OamDma::Idle {
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

    /// Reset a chaud : CPU, PPU, APU (E29a4/E30c1).
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.bus.ppu.reset();
        self.bus.apu.reset(); // E30c1
    }

    /// Boutons d'une manette (port 0 = $4016, port 1 = $4017), poses par le frontend (E24a2).
    pub fn set_buttons(&mut self, port: usize, buttons: u8) {
        self.bus.controller.set_buttons(port, buttons);
    }

    /// RAM de batterie de la cartouche (E29a1) ; None si la carte n'en a pas.
    pub fn battery_ram(&self) -> Option<&[u8]> {
        self.bus.mapper.battery_ram()
    }

    /// Restaure la RAM de batterie (contenu d'un .sav) (E29a1).
    pub fn load_battery_ram(&mut self, data: &[u8]) {
        self.bus.mapper.load_battery_ram(data);
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

    /// ROM minimale avec un programme a $8000 et vecteur RESET = $8000.
    fn rom_code(code: &[u8]) -> Vec<u8> {
        let mut rom = rom_minimale();
        let prg = 16; // offset du PRG dans le fichier
        rom[prg..prg + code.len()].copy_from_slice(code);
        rom[prg + 0x3FFC..prg + 0x3FFE].copy_from_slice(&[0x00, 0x80]); // RESET = $8000
        rom
    }

    fn ticks_jusqu_a_frontiere(nes: &mut Nes) {
        loop {
            nes.tick();
            if nes.cpu.at_instruction_boundary() {
                break;
            }
        }
    }

    fn ticks_dma(nes: &mut Nes) -> u64 {
        let s = nes.bus.cpu_cycles;
        while nes.bus.dma != OamDma::Idle {
            nes.tick();
        }
        nes.bus.cpu_cycles - s
    }

    #[test]
    fn dma_duree_513() {
        // STA $4014 (A = 0) : ecriture sur un cycle pair -> pas de cycle d'alignement.
        let mut nes = Nes::from_rom(&rom_code(&[0x8D, 0x14, 0x40])).unwrap();
        for _ in 0..7 {
            nes.tick(); // sequence de reset
        }
        assert_eq!(nes.bus.cpu_cycles, 7);
        ticks_jusqu_a_frontiere(&mut nes); // STA abs = 4 cycles
        assert_eq!(nes.bus.cpu_cycles, 11);
        assert_eq!(ticks_dma(&mut nes), 513);
    }

    #[test]
    fn dma_duree_514() {
        // LDA $00 ; STA $4014 : ecriture sur un cycle impair -> 1 cycle d'alignement.
        let mut nes = Nes::from_rom(&rom_code(&[0xA5, 0x00, 0x8D, 0x14, 0x40])).unwrap();
        for _ in 0..7 {
            nes.tick(); // sequence de reset
        }
        assert_eq!(nes.bus.cpu_cycles, 7);
        ticks_jusqu_a_frontiere(&mut nes); // LDA zp = 3 cycles
        assert_eq!(nes.bus.cpu_cycles, 10);
        ticks_jusqu_a_frontiere(&mut nes); // STA abs = 4 cycles
        assert_eq!(nes.bus.cpu_cycles, 14);
        assert_eq!(ticks_dma(&mut nes), 514);
    }

    #[test]
    fn dma_attend_lecture() {
        // INC $4014 : la lecture de $4014 renvoie l'open bus = $40 (octet haut de l'operande).
        let mut nes = Nes::from_rom(&rom_code(&[0xEE, 0x14, 0x40])).unwrap();
        for _ in 0..7 {
            nes.tick(); // sequence de reset
        }
        assert_eq!(nes.bus.cpu_cycles, 7);
        for _ in 0..5 {
            nes.tick(); // ecriture factice W($4014, $40) ; le CPU n'est pas arrete (ecriture a suivre)
        }
        assert_eq!(nes.bus.dma, OamDma::Requested(0x40));
        assert!(!nes.cpu.at_instruction_boundary());
        nes.tick(); // W($4014, $41) : la derniere ecriture gagne ; CPU a la frontiere
        assert_eq!(nes.bus.dma, OamDma::Requested(0x41));
        assert!(nes.cpu.at_instruction_boundary());
    }

    #[test]
    fn dma_step_instruction() {
        // STA $4014 : 4 cycles CPU + 513 cycles DMA.
        let mut nes = Nes::from_rom(&rom_code(&[0x8D, 0x14, 0x40])).unwrap();
        for _ in 0..7 {
            nes.tick(); // sequence de reset
        }
        assert_eq!(nes.step_instruction(), 517);
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

    // ---------- E28b1 : IRQ MMC3 sur le bus ----------

    /// ROM MMC3 (mapper 4) : PRG 32 Ko, CHR 8 Ko ; $E000 : JMP $E000 (banque fixe), RESET = $E000.
    fn rom_mmc3() -> Vec<u8> {
        let mut rom = vec![0x4E, 0x45, 0x53, 0x1A, 0x02, 0x01, 0x40, 0x00];
        rom.extend_from_slice(&[0; 8]);
        let mut prg = vec![0xEA; 32 * 1024];
        prg[0x6000..0x6003].copy_from_slice(&[0x4C, 0x00, 0xE0]);
        prg[0x7FFC..0x7FFE].copy_from_slice(&[0x00, 0xE0]);
        rom.extend(prg);
        rom.extend(vec![0; 8 * 1024]);
        rom
    }

    #[test]
    fn irq_mmc3_ligne_19() {
        let mut nes = Nes::from_rom(&rom_mmc3()).unwrap();
        nes.run_frame(); // s'arrete en (241, 1) : VBlank
        nes.bus.write(0x4017, 0x40); // E30c1 : IRQ de trame APU inhibee (sinon elle arrive avant)
        nes.bus.write(0x2000, 0x08); // sprites en $1000, fond en $0000
        nes.bus.write(0x2001, 0x18); // rendu actif
        nes.bus.write(0xC000, 20); // latch
        nes.bus.write(0xC001, 0); // rechargement au prochain clock (ligne 261)
        nes.bus.write(0xE001, 0); // IRQ activee
        assert!(!nes.bus.irq_line());
        let mut n = 0;
        while !nes.bus.irq_line() {
            nes.tick();
            n += 1;
            assert!(n < 100_000, "IRQ jamais levee");
        }
        let (line, point) = nes.bus.ppu.position();
        assert_eq!(line, 19, "IRQ en ({line}, {point})");
        assert!((255..=270).contains(&point), "IRQ en ({line}, {point})");
    }

    // ---------- E29a1 : RAM de batterie ----------

    #[test]
    fn nes_mmc1_batterie() {
        // MMC1 (octet 6 = $12 : mapper 1, batterie), PRG 32 Ko, CHR-RAM.
        let mut rom = vec![0x4E, 0x45, 0x53, 0x1A, 0x02, 0x00, 0x12, 0x00];
        rom.extend_from_slice(&[0; 8]);
        rom.extend(vec![0xEA; 32 * 1024]);
        let mut nes = Nes::from_rom(&rom).unwrap();
        assert_eq!(nes.battery_ram().map(|r| r.len()), Some(8 * 1024));
        nes.load_battery_ram(&[0xAA, 0xBB]);
        assert_eq!(nes.peek(0x6001), 0xBB);
        assert_eq!(nes.battery_ram().map(|r| r[0]), Some(0xAA));
    }

    #[test]
    fn nes_nrom_sans_batterie() {
        let nes = Nes::from_rom(&rom_minimale()).unwrap();
        assert!(nes.battery_ram().is_none());
    }

    // ---------- E33b2 : DMC DMA dans la machine ----------

    #[test]
    fn dmc_dma_cout() {
        // NOP partout ; DMC au debit maximal ($4010 = $0F : 54 cycles par bit, 432 par octet).
        let mut nes = Nes::from_rom(&rom_minimale()).unwrap();
        for _ in 0..7 {
            nes.tick(); // sequence de reset
        }
        nes.bus.write(0x4010, 0x0F);
        nes.bus.write(0x4013, 0x01); // 17 octets
        nes.bus.write(0x4015, 0x10);
        let mut voles = 0u64;
        for _ in 0..4000 {
            if nes.bus.dma_halts_cpu(nes.cpu.next_access_is_read()) {
                voles += 1;
            }
            nes.tick();
        }
        while nes.bus.dmc_dma != crate::dma::DmcDma::Idle {
            if nes.bus.dma_halts_cpu(nes.cpu.next_access_is_read()) {
                voles += 1; // termine la DMA en cours
            }
            nes.tick();
        }
        let octets = u64::from(17 - nes.bus.apu.dmc.remaining);
        assert!(octets >= 5, "octets = {octets}");
        assert!(
            (3 * octets..=4 * octets).contains(&voles),
            "voles = {voles}, octets = {octets}"
        );
    }
}
