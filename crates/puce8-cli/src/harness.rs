#![allow(dead_code)]
use puce8_core::nes::Nes;

#[derive(serde::Serialize)]
pub struct Resultat {
    pub rom: String,
    pub protocole: String,
    pub resultat: String, // REUSSI|ECHEC|TIMEOUT|ERREUR_ROM
    pub code: i32,
    pub texte: String,
    pub frames: u32,
    pub cycles: u64,
    pub hash: Option<String>,
}

fn make_result(
    rom: &str,
    protocole: &str,
    resultat: &str,
    code: i32,
    texte: &str,
    frames: u32,
    cycles: u64,
) -> Resultat {
    Resultat {
        rom: rom.to_string(),
        protocole: protocole.to_string(),
        resultat: resultat.to_string(),
        code,
        texte: texte.to_string(),
        frames,
        cycles,
        hash: None,
    }
}

pub fn read_c_string(nes: &Nes, addr: u16, max: usize) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < max {
        let byte = nes.peek(addr + i as u16);
        if byte == 0 {
            break;
        }
        result.push(byte as char);
        i += 1;
    }
    result
}

pub fn run_blargg6000(nes: &mut Nes, max_frames: u32) -> Resultat {
    let rom = "unknown".to_string(); // caller should set this via wrapper
    let mut frames = 0u32;
    let mut cycles = 0u64;

    let mut reset_pending = false;

    loop {
        if frames >= max_frames {
            return make_result(&rom, "blargg6000", "TIMEOUT", -1, "", frames, cycles);
        }

        // Check for signature at $6001-$6003: DE B0 61
        let sig_ok =
            nes.peek(0x6001) == 0xDE && nes.peek(0x6002) == 0xB0 && nes.peek(0x6003) == 0x61;

        if !sig_ok {
            // No signature yet, run one frame and check again
            let start = nes.bus.cpu_cycles;
            nes.run_frame();
            cycles += nes.bus.cpu_cycles - start;
            frames += 1;
            continue;
        }

        let status = nes.peek(0x6000);

        if status < 0x80 {
            // Done: 0 = success, otherwise error code
            let texte = read_c_string(nes, 0x6004, 256);
            if status == 0 {
                return make_result(&rom, "blargg6000", "REUSSI", 0, &texte, frames, cycles);
            } else {
                return make_result(
                    &rom,
                    "blargg6000",
                    "ECHEC",
                    status as i32,
                    &texte,
                    frames,
                    cycles,
                );
            }
        }

        // status >= 0x80 : attendre (jouer une image).
        if status == 0x81 && !reset_pending {
            // "Appuyer sur Reset, au moins 100 ms apres maintenant".
            // 7 images ~ 116 ms > 100 ms requis.
            for _ in 0..7 {
                let start = nes.bus.cpu_cycles;
                nes.run_frame();
                cycles += nes.bus.cpu_cycles - start;
                frames += 1;
            }
            nes.reset();
            reset_pending = true;
        } else {
            // status == 0x80 (attente), ou $81 deja traite, ou autre valeur >= 0x80.
            let start = nes.bus.cpu_cycles;
            nes.run_frame();
            cycles += nes.bus.cpu_cycles - start;
            frames += 1;
        }

        if status == 0x80 {
            reset_pending = false;
        }
    }
}

pub fn run_blargg_f8(nes: &mut Nes, frames: u32, result_addr: u16) -> Resultat {
    let rom = "unknown".to_string(); // caller should set this via wrapper
    let mut total_cycles = 0u64;

    for _ in 0..frames {
        let start = nes.bus.cpu_cycles;
        nes.run_frame();
        total_cycles += nes.bus.cpu_cycles - start;
    }

    let code = nes.peek(result_addr) as i32;
    if code == 1 {
        make_result(&rom, "blarggF8", "REUSSI", 1, "", frames, total_cycles)
    } else {
        make_result(&rom, "blarggF8", "ECHEC", code, "", frames, total_cycles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal iNES ROM with custom PRG code and proper reset vector.
    fn build_rom(prg: &[u8]) -> Vec<u8> {
        let mut rom = vec![0x4E, 0x45, 0x53, 0x1A]; // iNES magic
        rom.extend_from_slice(&[0x01, 0x00]); // 1 PRG bank (16K), 0 CHR banks
        rom.extend_from_slice(&[0; 10]); // bit7-4 flags6 + 8 reserved + bit3-0 flags7 (total header=16)

        let mut prg_data = vec![0xEA; 16384]; // fill with NOPs
                                              // Place test code at $8000 (offset 0 in PRG data)
        prg_data[..prg.len()].copy_from_slice(prg);

        // Set up reset vector at $FFFC-$FFFD -> $8000 (offsets 16380/16381 in prg_data)
        let vec_offset = 16384 - 4;
        prg_data[vec_offset] = 0x00; // low byte of $8000 ($FFFC)
        prg_data[vec_offset + 1] = 0x80; // high byte of $8000 ($FFFD)

        rom.extend_from_slice(&prg_data);
        rom
    }

    #[test]
    fn test_blargg6000_success() {
        // ROM writes blargg signature and signals success with text "OK"
        let prg = [
            0xA2, 0xDE, // LDX #$DE
            0x8E, 0x01, 0x60, // STX $6001
            0xA2, 0xB0, // LDX #$B0
            0x8E, 0x02, 0x60, // STX $6002
            0xA2, 0x61, // LDX #$61
            0x8E, 0x03, 0x60, // STX $6003
            0xA9, 0x00, // LDA #$00 (success)
            0x8D, 0x00, 0x60, // STA $6000
            0xA9, 0x4F, // LDA 'O'
            0x8D, 0x04, 0x60, // STA $6004
            0xA9, 0x4B, // LDA 'K'
            0x8D, 0x05, 0x60, // STA $6005
            0xA9, 0x00, // null terminator
            0x8D, 0x06, 0x60, // STA $6006
            0xFF, // BRA +0 (loop forever)
        ];

        let rom = build_rom(&prg);
        eprintln!("DEBUG: ROM size={}", rom.len());
        eprintln!("DEBUG: header bytes: {:02x} {:02x} {:02x} {:02x} | {:02x} {:02x} {:02x} {:02x} | {:02x} {:02x} {:02x} {:02x}",
                  rom[0], rom[1], rom[2], rom[3], rom[4], rom[5], rom[6], rom[7], rom[8], rom[9], rom[10], rom[11]);
        eprintln!(
            "DEBUG: bytes 12-20: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
            rom[12], rom[13], rom[14], rom[15], rom[16], rom[17], rom[18], rom[19], rom[20]
        );
        eprintln!(
            "DEBUG: vec bytes at rom[{}]={:#x} rom[{}]={:#x}",
            16 + 16380,
            rom[16 + 16380],
            16 + 16381,
            rom[16 + 16381]
        );
        let mut nes = Nes::from_rom(&rom).expect("valid ROM");
        eprintln!(
            "DEBUG: PC={:#06x} jammed={} cycles={}",
            nes.cpu.pc, nes.cpu.jammed, nes.cpu.cycles
        );
        // Run enough cycles for the code to execute (it's short)
        for _ in 0..100 {
            nes.tick();
        }
        eprintln!(
            "DEBUG: $6000={:#04x} $6001={:#04x} $6002={:#04x} $6003={:#04x}",
            nes.peek(0x6000),
            nes.peek(0x6001),
            nes.peek(0x6002),
            nes.peek(0x6003)
        );
        eprintln!(
            "DEBUG: PC={:#06x} jammed={} cycles={}",
            nes.cpu.pc, nes.cpu.jammed, nes.cpu.cycles
        );
        let result = run_blargg6000(&mut nes, 100);
        assert_eq!(result.resultat, "REUSSI");
        assert_eq!(result.code, 0);
        assert_eq!(result.texte, "OK");
    }

    #[test]
    fn test_blargg6000_failure() {
        // ROM signals failure with code 3
        let prg = [
            0xA2, 0xDE, // LDX #$DE
            0x8E, 0x01, 0x60, // STX $6001
            0xA2, 0xB0, // LDX #$B0
            0x8E, 0x02, 0x60, // STX $6002
            0xA2, 0x61, // LDX #$61
            0x8E, 0x03, 0x60, // STX $6003
            0xA9, 0x03, // LDA #$03 (error code)
            0x8D, 0x00, 0x60, // STA $6000
            0xFF, // BRA +0 (loop forever)
        ];

        let rom = build_rom(&prg);
        let mut nes = Nes::from_rom(&rom).expect("valid ROM");
        for _ in 0..100 {
            nes.tick();
        }
        let result = run_blargg6000(&mut nes, 100);
        assert_eq!(result.resultat, "ECHEC");
        assert_eq!(result.code, 3);
    }

    #[test]
    fn test_blargg6000_reset_timeout() {
        // ROM ecrit la signature blargg et garde $6000 = $81 en permanence :
        // le harnais doit finir en TIMEOUT (pas bloquer).
        let prg = [
            0xA2, 0xDE, // LDX #$DE
            0x8E, 0x01, 0x60, // STX $6001
            0xA2, 0xB0, // LDX #$B0
            0x8E, 0x02, 0x60, // STX $6002
            0xA2, 0x61, // LDX #$61
            0x8E, 0x03, 0x60, // STX $6003
            0xA9, 0x81, // LDA #$81
            0x8D, 0x00, 0x60, // STA $6000
            0xFF, // BRA +0 (boucle pour toujours)
        ];

        let rom = build_rom(&prg);
        let mut nes = Nes::from_rom(&rom).expect("valid ROM");
        for _ in 0..100 {
            nes.tick();
        }
        let result = run_blargg6000(&mut nes, 200);
        assert_eq!(result.resultat, "TIMEOUT");
    }

    #[test]
    fn test_read_c_string() {
        // ROM writes a string to $6010
        let prg = [
            0xA9, 0x48, // LDA 'H'
            0x8D, 0x10, 0x60, // STA $6010
            0xA9, 0x69, // LDA 'i'
            0x8D, 0x11, 0x60, // STA $6011
            0xA9, 0x00, // null
            0x8D, 0x12, 0x60, // STA $6012
            0xFF, // BRA +0 (loop forever)
        ];

        let rom = build_rom(&prg);
        let mut nes = Nes::from_rom(&rom).expect("valid ROM");
        for _ in 0..100 {
            nes.tick();
        }
        let s = read_c_string(&nes, 0x6010, 10);
        assert_eq!(s, "Hi");
    }

    #[test]
    fn test_blargg_f8_custom_addr() {
        // ROM ecrit son resultat a $F0 (pas $F8) : l'adresse est un parametre.
        let prg = [
            0xA9, 0x01, // LDA #$01
            0x85, 0xF0, // STA $F0
            0xFF, // BRA +0 (boucle pour toujours)
        ];

        let rom = build_rom(&prg);
        let mut nes = Nes::from_rom(&rom).expect("valid ROM");
        for _ in 0..5 {
            nes.run_frame();
        }
        let r = run_blargg_f8(&mut nes, 5, 0xF0);
        assert_eq!(r.resultat, "REUSSI");
        assert_eq!(r.code, 1);

        // A $F8 (valeur jamais ecrite) le resultat est ECHEC code 0.
        let mut nes = Nes::from_rom(&rom).expect("valid ROM");
        for _ in 0..5 {
            nes.run_frame();
        }
        let r = run_blargg_f8(&mut nes, 5, 0xF8);
        assert_eq!(r.resultat, "ECHEC");
        assert_eq!(r.code, 0);
    }
}
