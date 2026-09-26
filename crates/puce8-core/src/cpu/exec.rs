//! Exécution des micro-ops et des opérations (E04b : reset, NOP, JAM).

use super::micro_op::{Flow, MicroOp};
use super::operations::Operation;
use super::CpuBus;

/// Bits du registre P. B n'est jamais stocké ; U est toujours posé.
pub(crate) const FLAG_C: u8 = 0x01; // retenue
pub(crate) const FLAG_Z: u8 = 0x02; // zéro
pub(crate) const FLAG_I: u8 = 0x04; // masque IRQ (posé = masqué)
pub(crate) const FLAG_D: u8 = 0x08; // mode décimal
pub(crate) const FLAG_B: u8 = 0x10; // break — jamais stocké dans P
pub(crate) const FLAG_U: u8 = 0x20; // toujours posé
pub(crate) const FLAG_V: u8 = 0x40; // dépassement
pub(crate) const FLAG_N: u8 = 0x80; // négatif

// Disposition des bits de P (spécification E04b).
// (L'égalité de tuples n'est pas `const` en Rust stable : on compare octet par octet.)
const _: () = assert!(
    FLAG_C == 0x01
        && FLAG_Z == 0x02
        && FLAG_I == 0x04
        && FLAG_D == 0x08
        && FLAG_B == 0x10
        && FLAG_U == 0x20
        && FLAG_V == 0x40
        && FLAG_N == 0x80
);

impl super::Cpu {
    /// Exécute une micro-op : exactement un accès bus.
    pub(crate) fn run_micro_op(&mut self, m: MicroOp, bus: &mut impl CpuBus) -> Flow {
        match m {
            MicroOp::DummyReadPc => {
                let _ = bus.read(self.pc);
                Flow::Next
            }
            MicroOp::DummyReadPcExec => {
                let _ = bus.read(self.pc);
                self.exec_implied(self.op);
                Flow::Done
            }
            MicroOp::DummyReadStackDec => {
                let _ = bus.read(0x0100 + u16::from(self.s));
                self.s = self.s.wrapping_sub(1);
                Flow::Next
            }
            MicroOp::ReadVectorLo => {
                self.addr = u16::from(bus.read(self.vector));
                Flow::Next
            }
            MicroOp::ReadVectorHi => {
                let hi = bus.read(self.vector.wrapping_add(1));
                self.pc = (u16::from(hi) << 8) | self.addr;
                // E12 : si vector == $FFFA alors need_nmi = false.
                Flow::Done
            }
            MicroOp::Jam => {
                let _ = bus.read(0xFFFF);
                self.jammed = true;
                Flow::Done
            }
            other => unimplemented!("{:?}", other),
        }
    }

    /// « Quoi » d'une instruction implicite (E04b : NOP seulement).
    pub(crate) fn exec_implied(&mut self, op: Operation) {
        match op {
            Operation::Nop => {}
            other => unimplemented!("{:?}", other),
        }
    }

    /// « Quoi » d'une lecture (E05+).
    #[cfg_attr(not(test), expect(dead_code, reason = "utilisé à partir de E05"))]
    pub(crate) fn exec_read(&mut self, _op: Operation, _value: u8) {
        unimplemented!()
    }

    /// Valeur écrite par l'instruction (E06+).
    #[cfg_attr(not(test), expect(dead_code, reason = "utilisé à partir de E06"))]
    pub(crate) fn exec_write(&mut self, _op: Operation) -> u8 {
        unimplemented!()
    }

    /// Transformation lecture-modification-écriture (E08+).
    #[cfg_attr(not(test), expect(dead_code, reason = "utilisé à partir de E08"))]
    pub(crate) fn exec_rmw(&mut self, _op: Operation, _data: u8) -> u8 {
        unimplemented!()
    }

    // ---------- Aides sur les flags ----------

    /// Teste un bit de P.
    #[cfg_attr(not(test), expect(dead_code, reason = "utilisé à partir de E05"))]
    pub(crate) const fn flag(&self, mask: u8) -> bool {
        self.p & mask != 0
    }

    /// Pose ou efface un bit de P.
    pub(crate) fn set_flag(&mut self, mask: u8, on: bool) {
        if on {
            self.p |= mask;
        } else {
            self.p &= !mask;
        }
    }

    /// Pose N et Z à partir d'un octet.
    #[cfg_attr(not(test), expect(dead_code, reason = "utilisé à partir de E06"))]
    pub(crate) fn set_zn(&mut self, v: u8) {
        self.set_flag(FLAG_N, v & 0x80 != 0);
        self.set_flag(FLAG_Z, v == 0);
    }
}

#[cfg(test)]
mod tests {
    use super::{FLAG_N, FLAG_Z};
    use crate::cpu::Cpu;

    #[test]
    fn stubs_disponibles() {
        // Stubs E05/E06/E08 : présents mais pas encore appelés.
        let _ = (Cpu::exec_read, Cpu::exec_write, Cpu::exec_rmw);
    }

    #[test]
    fn set_zn_flags() {
        let mut cpu = Cpu::new();
        cpu.set_zn(0x81);
        assert!(cpu.flag(FLAG_N) && !cpu.flag(FLAG_Z));
        cpu.set_zn(0x00);
        assert!(!cpu.flag(FLAG_N) && cpu.flag(FLAG_Z));
    }
}
