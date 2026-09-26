//! Bus CPU : contrat vu par le processeur (E03) + bus de test pour les tests unitaires.

/// Contrat du bus vu par le 6502. Le bus ne fait pas avancer le temps :
/// un accès = un cycle, mais c'est `Cpu::tick()` (E04), puis `Nes::tick()`, qui cadence tout.
pub trait CpuBus {
    /// Lecture avec effets de bord (open bus, $2007, ...).
    fn read(&mut self, addr: u16) -> u8;
    /// Écriture avec effets de bord.
    fn write(&mut self, addr: u16, value: u8);
    /// Lecture sans aucun effet : même valeur que `read`, ni journal ni callback.
    fn peek(&self, addr: u16) -> u8;
    /// true = NMI demandée.
    fn nmi_line(&self) -> bool;
    /// true = IRQ active (sensible au niveau).
    fn irq_line(&self) -> bool;
}

pub mod micro_op;
pub mod opcodes;
pub mod operations;
pub mod ported_steps;

#[cfg(test)]
pub(crate) mod test_bus;

#[cfg(test)]
mod tests {
    use super::{micro_op::MicroOp, opcodes::*, ported_steps::*};

    #[test]
    fn tables_generees_coherentes() {
        let (mut off, mut jam) = (0, 0);
        for op in 0..=255u8 {
            let info = OPCODES[op as usize];
            if info.official {
                off += 1;
            }
            let s = ported_steps(op);
            if info.mnemonic == "JAM" {
                jam += 1;
                continue;
            }
            let early = s.contains(&MicroOp::ReadIndexedPageCheck) as usize;
            let base = if info.mode == Mode::Rel {
                s.len() - 1
            } else {
                1 + s.len() - early
            };
            assert_eq!(base as u8, info.cycles, "opcode {:02X}", op);
        }
        assert_eq!((off, jam), (151, 12));
        assert_eq!(RESET_SEQ.len(), 7);
        assert_eq!(INTERRUPT_SEQ.len(), 6);
    }
}
