//! Aides de test du CPU (E05a) : exécuter un court programme à $0600 et inspecter le journal.

use super::test_bus::TestBus;
use super::Cpu;

/// Exécute `code` en $0600 après reset (vecteur = $0600), applique `setup`, puis fait
/// l'instruction complète. Après `clear_log()`, `log[0]` est la lecture de l'opcode.
pub(crate) fn run(code: &[u8], setup: impl FnOnce(&mut Cpu, &mut TestBus)) -> (Cpu, TestBus) {
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
    let _ticks = cpu.step_instruction(&mut bus);
    (cpu, bus)
}

#[cfg(test)]
mod t1 {
    use super::run;
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
}
