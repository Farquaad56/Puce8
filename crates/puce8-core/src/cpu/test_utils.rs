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
    use crate::cpu::exec::{FLAG_C, FLAG_D, FLAG_N, FLAG_V, FLAG_Z};
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

    // --- E06a : chargements, stockages, transferts ---

    #[test]
    fn lda_zero() {
        let (cpu, _) = run(&[0xA9, 0x00], |_, _| {});
        assert!(cpu.flag(FLAG_Z));
        assert!(!cpu.flag(FLAG_N));
    }

    #[test]
    fn ldx_zpy() {
        let (cpu, bus) = run(&[0xB6, 0x10], |cpu, bus| {
            cpu.y = 5;
            bus.load(0x15, &[7]);
        });
        assert_eq!(bus.log.len(), 4); // opcode + FetchZp + DummyReadZpAddY + ReadExec
        assert_eq!(cpu.x, 7);
    }

    #[test]
    fn sty_zpx() {
        let (_, bus) = run(&[0x94, 0x10], |cpu, _| {
            cpu.x = 1;
            cpu.y = 3;
        });
        assert_eq!(bus.log.last(), Some(&Access::Write(0x0011, 3)));
    }

    #[test]
    fn tsx_flags() {
        let (cpu, _) = run(&[0xBA], |cpu, _| cpu.s = 0x80);
        assert_eq!(cpu.x, 0x80);
        assert!(cpu.flag(FLAG_N));
    }

    #[test]
    fn txs_sans_flags() {
        let (cpu, _) = run(&[0x9A], |cpu, _| {
            cpu.x = 0;
            cpu.set_zn(0x85); // N = 1, Z = 0 : Txs ne doit rien toucher
        });
        assert_eq!(cpu.s, 0);
        assert!(cpu.flag(FLAG_N));
        assert!(!cpu.flag(FLAG_Z));
    }

    #[test]
    fn invariants_e06() {
        verifier_invariants(&[
            0xA9, 0xA5, 0xB5, 0xAD, 0xBD, 0xB9, 0xA1, 0xB1, // LDA
            0xA2, 0xA6, 0xB6, 0xAE, 0xBE, // LDX
            0xA0, 0xA4, 0xB4, 0xAC, 0xBC, // LDY
            0x85, 0x95, 0x8D, 0x9D, 0x99, 0x81, 0x91, // STA
            0x86, 0x96, 0x8E, // STX
            0x84, 0x94, 0x8C, // STY
            0xAA, 0xA8, 0x8A, 0x98, 0xBA, 0x9A, // transferts
        ]);
    }

    // --- E07a : arithmétique et logique ---

    #[test]
    fn adc_table() {
        // (A, M, Cin) → (A, C, V).
        let cases = [
            (0x50u8, 0x10u8, false, 0x60u8, false, false),
            (0x50, 0x50, false, 0xA0, false, true),
            (0xFF, 0x01, false, 0x00, true, false),
            (0x80, 0xFF, false, 0x7F, true, true),
            (0x00, 0x00, true, 0x01, false, false),
        ];
        for (a, m, cin, ra, rc, rv) in cases {
            let (cpu, _) = run(&[0x69, m], |cpu, _| {
                cpu.a = a;
                cpu.set_flag(FLAG_C, cin);
            });
            assert_eq!(cpu.a, ra, "A={:02X} M={:02X} C={}", a, m, cin);
            assert_eq!(
                cpu.flag(FLAG_C),
                rc,
                "C : A={:02X} M={:02X} C={}",
                a,
                m,
                cin
            );
            assert_eq!(
                cpu.flag(FLAG_V),
                rv,
                "V : A={:02X} M={:02X} C={}",
                a,
                m,
                cin
            );
        }
        // (FF, 01, 0) → Z = 1.
        let (cpu, _) = run(&[0x69, 0x01], |cpu, _| {
            cpu.a = 0xFF;
            cpu.set_flag(FLAG_C, false);
        });
        assert!(cpu.flag(FLAG_Z));
    }

    #[test]
    fn sbc_table() {
        // (A, M, Cin) → (A, C, V).
        let cases = [
            (0x50u8, 0xF0u8, true, 0x60u8, false, false),
            (0x50, 0xB0, true, 0xA0, false, true),
            (0xD0, 0x70, true, 0x60, true, true),
            (0x05, 0x05, false, 0xFF, false, false),
        ];
        for (a, m, cin, ra, rc, rv) in cases {
            let (cpu, _) = run(&[0xE9, m], |cpu, _| {
                cpu.a = a;
                cpu.set_flag(FLAG_C, cin);
            });
            assert_eq!(cpu.a, ra, "A={:02X} M={:02X} C={}", a, m, cin);
            assert_eq!(
                cpu.flag(FLAG_C),
                rc,
                "C : A={:02X} M={:02X} C={}",
                a,
                m,
                cin
            );
            assert_eq!(
                cpu.flag(FLAG_V),
                rv,
                "V : A={:02X} M={:02X} C={}",
                a,
                m,
                cin
            );
        }
    }

    #[test]
    fn mode_decimal_ignore() {
        // D = 1 : pas de mode décimal sur le 2A03 — ADC reste binaire.
        let (cpu, _) = run(&[0x69, 0x01], |cpu, _| {
            cpu.a = 0x09;
            cpu.set_flag(FLAG_D, true);
        });
        assert_eq!(cpu.a, 0x0A);
    }

    #[test]
    fn and_zero() {
        let (cpu, _) = run(&[0x29, 0x0F], |cpu, _| cpu.a = 0xF0);
        assert_eq!(cpu.a, 0);
        assert!(cpu.flag(FLAG_Z));
    }

    #[test]
    fn invariants_e07a() {
        verifier_invariants(&[
            0x69, 0x65, 0x75, 0x6D, 0x7D, 0x79, 0x61, 0x71, // ADC
            0xE9, 0xE5, 0xF5, 0xED, 0xFD, 0xF9, 0xE1, 0xF1, // SBC
            0x29, 0x25, 0x35, 0x2D, 0x3D, 0x39, 0x21, 0x31, // AND
            0x09, 0x05, 0x15, 0x0D, 0x1D, 0x19, 0x01, 0x11, // ORA
            0x49, 0x45, 0x55, 0x4D, 0x5D, 0x59, 0x41, 0x51, // EOR
        ]);
    }
}
