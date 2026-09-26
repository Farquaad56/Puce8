//! Aides de test du CPU (E05a/E05b) : exécuter un court programme à $0600 et inspecter le journal.

use super::test_bus::TestBus;
use super::Cpu;
use crate::cpu::opcodes::{Mode, OPCODES};

/// Exécute `code` en $0600 après reset (vecteur = $0600), applique `setup`, puis fait
/// l'instruction complète. Après `clear_log()`, `log[0]` est la lecture de l'opcode.
pub(crate) fn run(code: &[u8], setup: impl FnOnce(&mut Cpu, &mut TestBus)) -> (Cpu, TestBus) {
    let (cpu, bus, _ticks) = run_ticks(code, setup);
    (cpu, bus)
}

/// Comme `run`, mais renvoie aussi le nombre de ticks de l'instruction.
fn run_ticks(code: &[u8], setup: impl FnOnce(&mut Cpu, &mut TestBus)) -> (Cpu, TestBus, u32) {
    let mut bus = TestBus::new();
    bus.set_vector(0xFFFC, 0x0600);
    bus.load(0x0600, code);
    let mut cpu = Cpu::new();
    for _ in 0..7 {
        cpu.tick(&mut bus);
    }
    assert!(cpu.at_instruction_boundary());
    bus.clear_log();
    setup(&mut cpu, &mut bus);
    let ticks = cpu.step_instruction(&mut bus);
    (cpu, bus, ticks)
}

/// Pour chaque opcode : opérandes $10 $02, X = Y = 0, pointeurs de page zéro ($10/$11) → $0200.
/// Vérifie : chaque tick = exactement 1 accès, et le nombre de ticks == OPCODES[op].cycles.
/// Puis, si page_penalty == 1 : X = Y = $FF, base $02F0 → cycles + 1.
pub(crate) fn verifier_invariants(ops: &[u8]) {
    for &op in ops {
        let info = OPCODES[op as usize];
        // Phase 1 : opérandes $10 $02, X = Y = 0 (défaut après reset), pointeurs → $0200.
        let code = match info.mode.operand_len() {
            0 => vec![op],
            1 => vec![op, 0x10],
            _ => vec![op, 0x10, 0x02],
        };
        let (_, bus, ticks) = run_ticks(&code, |_, bus| {
            if matches!(info.mode, Mode::Izx | Mode::Izy) {
                bus.load(0x10, &[0x00, 0x02]); // pointeurs $10/$11 → $0200
            }
        });
        assert_eq!(
            bus.log.len(),
            ticks as usize,
            "opcode {:02X} : 1 accès par tick",
            op
        );
        assert_eq!(ticks, info.cycles as u32, "opcode {:02X}", op);

        // Phase 2 : si page_penalty == 1 : X = Y = $FF, base $02F0 → cycles + 1.
        if info.page_penalty == 1 {
            let code = match info.mode {
                Mode::Izy => vec![op, 0x10], // pointeurs $10/$11 → base $02F0
                _ => vec![op, 0xF0, 0x02],
            };
            let (_, bus, ticks) = run_ticks(&code, |cpu, bus| {
                cpu.x = 0xFF;
                cpu.y = 0xFF;
                if info.mode == Mode::Izy {
                    bus.load(0x10, &[0xF0, 0x02]); // pointeurs $10/$11 → base $02F0
                }
            });
            assert_eq!(
                bus.log.len(),
                ticks as usize,
                "opcode {:02X} : 1 accès par tick",
                op
            );
            assert_eq!(
                ticks,
                info.cycles.wrapping_add(1) as u32,
                "opcode {:02X}",
                op
            );
        }
    }
}

#[cfg(test)]
mod t1 {
    use super::{run, verifier_invariants};
    use crate::cpu::exec::{FLAG_N, FLAG_Z};
    use crate::cpu::test_bus::Access;

    #[test]
    fn lda_imm() {
        let (cpu, bus) = run(&[0xA9, 0x80], |_, _| {});
        assert_eq!(bus.log.len(), 2); // opcode + ReadImmExec
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.flag(FLAG_N));
        assert!(!cpu.flag(FLAG_Z));
    }

    #[test]
    fn zp0_lda() {
        let (cpu, bus) = run(&[0xA5, 0x10], |_, bus| bus.load(0x10, &[7]));
        assert_eq!(
            &bus.log[1..],
            &[Access::Read(0x0601, 0x10), Access::Read(0x0010, 7)]
        );
        assert_eq!(cpu.a, 7);
    }

    #[test]
    fn zpx_wrap() {
        let (_, bus) = run(&[0xB5, 0xFF], |cpu, _| cpu.x = 2);
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0xFF),
                Access::Read(0x00FF, 0),
                Access::Read(0x0001, 0)
            ]
        );
    }

    #[test]
    fn abx_sans_page() {
        let (_, bus) = run(&[0xBD, 0x10, 0x12], |cpu, _| cpu.x = 1);
        assert_eq!(bus.log.len(), 4); // pas de franchissement : ReadIndexedPageCheck termine l'instruction
        assert_eq!(bus.log.last(), Some(&Access::Read(0x1211, 0)));
    }

    #[test]
    fn abx_avec_page() {
        let (_, bus) = run(&[0xBD, 0xFF, 0x12], |cpu, _| cpu.x = 1);
        assert_eq!(bus.log.len(), 5); // franchissement de page : +1 cycle
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0xFF),
                Access::Read(0x0602, 0x12),
                Access::Read(0x1200, 0),
                Access::Read(0x1300, 0)
            ]
        );
    }

    #[test]
    fn izx() {
        let (_, bus) = run(&[0xA1, 0x20], |cpu, bus| {
            cpu.x = 4;
            bus.load(0x24, &[0x00, 0x80]);
        });
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0x20),
                Access::Read(0x0020, 0),
                Access::Read(0x0024, 0x00),
                Access::Read(0x0025, 0x80),
                Access::Read(0x8000, 0)
            ]
        );
    }

    #[test]
    fn izx_wrap() {
        let (_, bus) = run(&[0xA1, 0xFF], |cpu, bus| {
            cpu.x = 0;
            bus.load(0xFF, &[0x34]);
            bus.load(0x00, &[0x12]);
        });
        assert_eq!(bus.log.last(), Some(&Access::Read(0x1234, 0))); // lecture en $1234
    }

    #[test]
    fn izy_page() {
        let (_, bus) = run(&[0xB1, 0x40], |cpu, bus| {
            cpu.y = 1;
            bus.load(0x40, &[0xFF, 0x10]);
        });
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0x40),
                Access::Read(0x0040, 0xFF),
                Access::Read(0x0041, 0x10),
                Access::Read(0x1000, 0),
                Access::Read(0x1100, 0)
            ]
        );
    }

    #[test]
    fn sta_abs() {
        let (_, bus) = run(&[0x8D, 0x34, 0x12], |cpu, _| cpu.a = 9);
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0x34),
                Access::Read(0x0602, 0x12),
                Access::Write(0x1234, 9)
            ]
        );
    }

    #[test]
    fn sta_abx_factice() {
        let (_, bus) = run(&[0x9D, 0x10, 0x12], |cpu, _| {
            cpu.a = 9;
            cpu.x = 1;
        });
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0x10),
                Access::Read(0x0602, 0x12),
                Access::Read(0x1211, 0),
                Access::Write(0x1211, 9)
            ]
        );
    }

    #[test]
    fn inc_zp() {
        let (_, bus) = run(&[0xE6, 0x10], |_, bus| bus.load(0x10, &[5]));
        assert_eq!(
            &bus.log[1..],
            &[
                Access::Read(0x0601, 0x10),
                Access::Read(0x0010, 5),
                Access::Write(0x0010, 5),
                Access::Write(0x0010, 6)
            ]
        );
    }

    #[test]
    fn inc_abx_7() {
        let (_, bus) = run(&[0xFE, 0x00, 0x02], |cpu, _| cpu.x = 1);
        assert_eq!(bus.log.len(), 7); // 7 ticks : chaque tick = exactement 1 accès
        assert_eq!(
            &bus.log[3..],
            &[
                Access::Read(0x0201, 0),
                Access::Read(0x0201, 0),
                Access::Write(0x0201, 0), // ancienne valeur
                Access::Write(0x0201, 1)  // nouvelle valeur
            ]
        );
    }

    #[test]
    fn invariants_sondes() {
        verifier_invariants(&[
            0xA9, 0xA5, 0xB5, 0xAD, 0xBD, 0xB9, 0xA1, 0xB1, 0x85, 0x95, 0x8D, 0x9D, 0x99, 0x81,
            0x91, 0xE6, 0xF6, 0xEE, 0xFE,
        ]);
    }
}
