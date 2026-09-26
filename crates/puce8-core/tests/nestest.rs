//! nestest (E10b) : les 5 003 premières lignes de `tests/roms/other/nestest.log`
//! (opcodes officiels) doivent être identiques à notre trace, PPU non comparé.
//! E11c : `nesttest_complet` compare les 8 991 lignes du log et vérifie les codes d'erreur.

use puce8_core::bus::Bus;
use puce8_core::cartridge::Cartridge;
use puce8_core::cpu::trace::{capture, compare_with_reference, format_nestest};
use puce8_core::cpu::{Cpu, CpuBus};
use puce8_core::mapper::create_mapper;

/// Dernière ligne de la section « opcodes officiels » (la 5 004 est le premier opcode non officiel).
const LAST_OFFICIAL_LINE: usize = 5003;

/// Toutes les lignes du log (E11c : nesttest complet, opcodes non officiels inclus).
const LAST_LINE: usize = 8991;

#[test]
fn nestest_officiels() {
    let rom_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/roms/other/nestest.nes"
    );
    let Ok(rom) = std::fs::read(rom_path) else {
        eprintln!("nesttest_officiels ignoré : {rom_path} absent");
        return;
    };
    let log_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/roms/other/nestest.log"
    );
    let Ok(log) = std::fs::read_to_string(log_path) else {
        eprintln!("nesttest_officiels ignoré : {log_path} absent");
        return;
    };

    // NROM (mapper 0) : PRG-ROM en $8000-$FFFF.
    let cart = Cartridge::from_bytes(&rom).expect("nestest.nes est une ROM iNES valide");
    assert_eq!(cart.mapper_id, 0);
    let mut bus = Bus::new(create_mapper(cart).expect("mapper NROM"));

    // Reset à froid : la séquence de reset compte 7 ticks.
    let mut cpu = Cpu::new();
    for _ in 0..7 {
        cpu.tick(&mut bus);
    }
    assert_eq!(cpu.cycles, 7);
    cpu.pc = 0xC000;

    let lines: Vec<&str> = log.lines().collect();
    assert!(lines.len() >= LAST_OFFICIAL_LINE, "nestest.log trop court");

    // Les 5 lignes obtenues précédemment, pour le message de panique.
    let mut prev: Vec<String> = Vec::new();
    for (i, line) in lines[..LAST_OFFICIAL_LINE].iter().enumerate() {
        let t = capture(&cpu, &bus);
        if let Err(e) = compare_with_reference(line, &t, false) {
            panic!(
                "nestest ligne {} : {e}\nattendue : {}\nobtenue  : {}\nlignes précédentes :\n{}",
                i + 1,
                line,
                format_nestest(&t),
                prev.join("\n")
            );
        }
        prev.push(format_nestest(&t));
        if prev.len() > 5 {
            prev.remove(0);
        }
        cpu.step_instruction(&mut bus);
    }
}

#[test]
fn nesttest_complet() {
    let rom_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/roms/other/nestest.nes"
    );
    let Ok(rom) = std::fs::read(rom_path) else {
        eprintln!("nesttest_complet ignoré : {rom_path} absent");
        return;
    };
    let log_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/roms/other/nestest.log"
    );
    let Ok(log) = std::fs::read_to_string(log_path) else {
        eprintln!("nesttest_complet ignoré : {log_path} absent");
        return;
    };

    // NROM (mapper 0) : PRG-ROM en $8000-$FFFF.
    let cart = Cartridge::from_bytes(&rom).expect("nestest.nes est une ROM iNES valide");
    assert_eq!(cart.mapper_id, 0);
    let mut bus = Bus::new(create_mapper(cart).expect("mapper NROM"));

    // Reset à froid : la séquence de reset compte 7 ticks.
    let mut cpu = Cpu::new();
    for _ in 0..7 {
        cpu.tick(&mut bus);
    }
    assert_eq!(cpu.cycles, 7);
    cpu.pc = 0xC000;

    let lines: Vec<&str> = log.lines().collect();
    assert!(lines.len() >= LAST_LINE, "nestest.log trop court");

    // Les 5 lignes obtenues précédemment, pour le message de panique.
    let mut prev: Vec<String> = Vec::new();
    for (i, line) in lines[..LAST_LINE].iter().enumerate() {
        let t = capture(&cpu, &bus);
        if let Err(e) = compare_with_reference(line, &t, false) {
            panic!(
                "nestest ligne {} : {e}\nattendue : {}\nobtenue  : {}\nlignes précédentes :\n{}",
                i + 1,
                line,
                format_nestest(&t),
                prev.join("\n")
            );
        }
        prev.push(format_nestest(&t));
        if prev.len() > 5 {
            prev.remove(0);
        }
        cpu.step_instruction(&mut bus);
    }

    // Codes d'erreur de nestest : aucun des deux ne doit être posé.
    assert_eq!(bus.peek(0x0002), 0);
    assert_eq!(bus.peek(0x0003), 0);
}
