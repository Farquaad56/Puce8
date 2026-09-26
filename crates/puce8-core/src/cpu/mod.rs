//! Bus CPU : contrat vu par le processeur (E03) + séquenceur cycle-exact `Cpu` (E04).

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
pub mod trace;

mod exec;

#[cfg(test)]
pub(crate) mod test_bus;

#[cfg(test)]
pub(crate) mod test_utils;

use micro_op::{Flow, MicroOp};
use operations::Operation;
use ported_steps::{ported_steps, INTERRUPT_SEQ, RESET_SEQ};

/// Processeur 6502 : séquenceur cycle-exact (1 tick = 1 cycle = 1 accès bus).
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub pc: u16,
    pub p: u8,
    /// Nombre total de cycles exécutés (la séquence de reset compte).
    pub cycles: u64,
    /// JAM : le CPU lit $FFFF à chaque cycle, pour toujours.
    pub jammed: bool,

    steps: &'static [MicroOp],
    idx: usize,
    pub(crate) opcode: u8,
    pub(crate) op: Operation,
    pub(crate) addr: u16,
    pub(crate) base: u16,
    pub(crate) ptr: u8,
    pub(crate) data: u8,
    pub(crate) crossed: bool,
    pub(crate) vector: u16,

    // Interruptions (remplies en E12) :
    pub(crate) prev_nmi_line: bool,
    pub(crate) need_nmi: bool,
    pub(crate) prev_need_nmi: bool,
    pub(crate) run_irq: bool,
    pub(crate) prev_run_irq: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    /// Reset à froid : A = X = Y = 0, S = 0, P = $24 (U et I posés).
    /// Les 7 premiers ticks exécutent la séquence de reset.
    pub fn new() -> Self {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            s: 0,
            pc: 0,
            p: 0x24,
            cycles: 0,
            jammed: false,
            steps: RESET_SEQ,
            idx: 0,
            opcode: 0,
            op: Operation::Nop,
            addr: 0,
            base: 0,
            ptr: 0,
            data: 0,
            crossed: false,
            vector: 0xFFFC,
            prev_nmi_line: false,
            need_nmi: false,
            prev_need_nmi: false,
            run_irq: false,
            prev_run_irq: false,
        }
    }

    /// Reset à chaud : A, X, Y sont conservés ; I est posé.
    pub fn reset(&mut self) {
        self.steps = RESET_SEQ;
        self.idx = 0;
        self.vector = 0xFFFC;
        self.set_flag(exec::FLAG_I, true);
    }

    /// Exécute un cycle (exactement un accès bus).
    pub fn tick(&mut self, bus: &mut impl CpuBus) {
        if self.jammed {
            let _ = bus.read(0xFFFF);
            self.cycles += 1;
            return;
        }
        if self.idx >= self.steps.len() {
            // Frontière d'instruction.
            if self.prev_need_nmi || self.prev_run_irq {
                self.begin_interrupt(bus);
            } else {
                self.opcode = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.steps = ported_steps(self.opcode);
                self.op = operations::operation(self.opcode);
                self.idx = 0;
            }
        } else {
            let m = self.steps[self.idx];
            self.idx += 1;
            if self.run_micro_op(m, bus) == Flow::Done {
                self.idx = self.steps.len();
            }
        }
        self.cycles += 1;
        self.sample_interrupts(bus);
    }

    /// Vrai à une frontière d'instruction (le tick suivant lit l'opcode ou gère une interruption).
    pub fn at_instruction_boundary(&self) -> bool {
        self.idx >= self.steps.len()
    }

    /// Exécute une instruction complète (lecture d'opcode comprise) jusqu'à la
    /// frontière suivante ; renvoie le nombre de ticks.
    pub fn step_instruction(&mut self, bus: &mut impl CpuBus) -> u32 {
        // Au moins un tick : à la frontière, le premier tick lit l'opcode.
        let mut n = 0u32;
        loop {
            self.tick(bus);
            n += 1;
            if self.at_instruction_boundary() {
                return n;
            }
        }
    }

    /// Vrai si le prochain accès bus est une lecture (la DMA peut préempter).
    pub fn next_access_is_read(&self) -> bool {
        if self.at_instruction_boundary() {
            true // Lecture d'opcode (ou d'interruption, ou JAM).
        } else {
            !self.steps[self.idx].is_write()
        }
    }

    /// Début de séquence NMI/IRQ (E12a) : 1er cycle = R*(PC), à la place de la lecture
    /// de l'opcode ; PC n'est pas incrémenté.
    fn begin_interrupt(&mut self, bus: &mut impl CpuBus) {
        let _ = bus.read(self.pc); // lecture factice R*(PC), PC non incrémenté
        self.steps = INTERRUPT_SEQ;
        self.idx = 0;
    }

    /// Échantillonne les lignes NMI/IRQ en fin de cycle (E12a).
    fn sample_interrupts(&mut self, bus: &impl CpuBus) {
        self.prev_need_nmi = self.need_nmi;
        let nmi = bus.nmi_line();
        if nmi && !self.prev_nmi_line {
            self.need_nmi = true; // front montant
        }
        self.prev_nmi_line = nmi;
        self.prev_run_irq = self.run_irq;
        self.run_irq = bus.irq_line() && ((self.p & exec::FLAG_I) == 0); // niveau, masqué par I
    }
}

#[cfg(test)]
mod tests {
    use super::{exec, Cpu};
    use super::{micro_op::MicroOp, opcodes::*, ported_steps::*};
    use crate::cpu::test_bus::{Access, TestBus};

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

    /// Exécute la séquence de reset (vecteur $FFFC → `target`).
    fn run_reset(cpu: &mut Cpu, bus: &mut TestBus) {
        for _ in 0..7 {
            cpu.tick(bus);
        }
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn reset_7_ticks() {
        let mut bus = TestBus::new();
        bus.set_vector(0xFFFC, 0x8000);
        let mut cpu = Cpu::new();
        run_reset(&mut cpu, &mut bus);
        assert_eq!(cpu.cycles, 7);
        // 7 ticks → 7 accès, tous des lectures.
        assert_eq!(bus.log.len(), 7);
        for a in &bus.log {
            assert!(
                matches!(a, Access::Read(_, _)),
                "écriture pendant le reset : {a:?}"
            );
        }
        assert_eq!(
            &bus.log[2..5],
            &[
                Access::Read(0x0100, 0),
                Access::Read(0x01FF, 0),
                Access::Read(0x01FE, 0)
            ]
        );
        assert_eq!(cpu.pc, 0x8000);
        assert_eq!(cpu.s, 0xFD);
        assert_eq!(cpu.p, 0x24);
    }

    #[test]
    fn reset_a_chaud() {
        let mut bus = TestBus::new();
        bus.set_vector(0xFFFC, 0x8000);
        let mut cpu = Cpu::new();
        run_reset(&mut cpu, &mut bus);
        cpu.a = 5;
        cpu.reset();
        for _ in 0..7 {
            cpu.tick(&mut bus);
        }
        assert_eq!(cpu.a, 5); // A conservé
        assert_eq!(cpu.s, 0xFA);
        assert_ne!(cpu.p & exec::FLAG_I, 0); // I = 1
    }

    #[test]
    fn nop_2_ticks() {
        let mut bus = TestBus::new();
        bus.set_vector(0xFFFC, 0x8000);
        bus.load(0x8000, &[0xEA]);
        let mut cpu = Cpu::new();
        run_reset(&mut cpu, &mut bus);
        bus.clear_log();
        assert_eq!(cpu.step_instruction(&mut bus), 2);
        assert_eq!(
            bus.log,
            vec![Access::Read(0x8000, 0xEA), Access::Read(0x8001, 0)]
        );
        assert_eq!(cpu.pc, 0x8001);
    }

    #[test]
    fn un_tick_un_acces() {
        let mut bus = TestBus::new();
        bus.set_vector(0xFFFC, 0x8000);
        bus.load(0x8000, &[0xEAu8; 32]);
        let mut cpu = Cpu::new();
        run_reset(&mut cpu, &mut bus);
        bus.clear_log();
        for _ in 0..20 {
            cpu.tick(&mut bus);
        }
        assert_eq!(bus.log.len(), 20);
    }

    #[test]
    fn jam_bloque() {
        let mut bus = TestBus::new();
        bus.set_vector(0xFFFC, 0x8000);
        bus.load(0x8000, &[0x02]); // JAM
        let mut cpu = Cpu::new();
        run_reset(&mut cpu, &mut bus);
        bus.clear_log();
        assert_eq!(cpu.step_instruction(&mut bus), 2); // opcode + Jam
        assert!(cpu.jammed);
        let pc = cpu.pc;
        for _ in 0..5 {
            cpu.tick(&mut bus);
        }
        assert_eq!(&bus.log[2..], &[Access::Read(0xFFFF, 0); 5]); // 1 accès par tick
        assert_eq!(cpu.pc, pc); // PC figé
    }

    #[test]
    fn etat_initial() {
        let cpu = Cpu::new();
        assert_eq!((cpu.a, cpu.x, cpu.y, cpu.s), (0, 0, 0, 0));
        assert_eq!(cpu.p, 0x24);
        assert!(cpu.flag(exec::FLAG_I) && cpu.flag(exec::FLAG_U));
        assert!(!cpu.jammed);
        assert_eq!(cpu.cycles, 0);
        // Champs réservés aux étapes suivantes (E05/E12).
        assert_eq!((cpu.base, cpu.ptr, cpu.data), (0, 0, 0));
        assert!(!cpu.crossed);
        assert_eq!(cpu.vector, 0xFFFC);
        assert!(!cpu.prev_nmi_line && !cpu.need_nmi && !cpu.run_irq);
    }
}
