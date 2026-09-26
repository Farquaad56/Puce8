//! Exécution des micro-ops et des opérations (E04b : reset, NOP, JAM ; E05a : adressage en lecture + LDA ; E05b : écriture/RMW + sondes STA/INC ; E06a : load/store/transferts ; E07a : ADC/SBC/AND/ORA/EOR ; E07b : CMP/CPX/CPY, BIT, INX/INY/DEX/DEY ; E08a : ASL/LSR/ROL/ROR A ; E08b : RMW mémoire + DEC ; E09a : branchements et JMP ; E09b : JSR/RTS/pile/instructions de flags).

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
            // --- E05a : adressage en lecture (1 accès bus par micro-op) ---
            MicroOp::ReadImmExec => {
                let v = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.exec_read(self.op, v);
                Flow::Done
            }
            MicroOp::FetchZp | MicroOp::FetchAbsLo => {
                self.addr = u16::from(bus.read(self.pc));
                self.pc = self.pc.wrapping_add(1);
                Flow::Next
            }
            MicroOp::DummyReadZpAddX => {
                let _ = bus.read(self.addr);
                self.addr = self.addr.wrapping_add(u16::from(self.x)) & 0xFF;
                Flow::Next
            }
            MicroOp::DummyReadZpAddY => {
                let _ = bus.read(self.addr);
                self.addr = self.addr.wrapping_add(u16::from(self.y)) & 0xFF;
                Flow::Next
            }
            MicroOp::FetchAbsHi => {
                let hi = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.addr = self.addr.wrapping_add(u16::from(hi) << 8);
                Flow::Next
            }
            MicroOp::FetchAbsHiAddX => self.fetch_abs_hi_add(self.x, bus),
            MicroOp::FetchAbsHiAddY => self.fetch_abs_hi_add(self.y, bus),
            MicroOp::FetchPtr => {
                self.ptr = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                Flow::Next
            }
            MicroOp::DummyReadPtrAddX => {
                let _ = bus.read(u16::from(self.ptr));
                self.ptr = self.ptr.wrapping_add(self.x);
                Flow::Next
            }
            MicroOp::ReadPtrLo => {
                self.addr = u16::from(bus.read(u16::from(self.ptr)));
                Flow::Next
            }
            MicroOp::ReadPtrHi => {
                let hi = bus.read(u16::from(self.ptr.wrapping_add(1)));
                self.addr = self.addr.wrapping_add(u16::from(hi) << 8);
                Flow::Next
            }
            MicroOp::ReadPtrHiAddY => {
                let hi = bus.read(u16::from(self.ptr.wrapping_add(1)));
                let base = (u16::from(hi) << 8) | self.addr;
                self.base = base;
                self.addr = base.wrapping_add(u16::from(self.y));
                self.crossed = (base ^ self.addr) & 0xFF00 != 0;
                Flow::Next
            }
            MicroOp::ReadIndexedPageCheck => {
                if self.crossed {
                    // Franchissement de page : cycle de pénalité sur l'adresse non corrigée.
                    let _ = bus.read((self.base & 0xFF00) | (self.addr & 0xFF));
                    Flow::Next
                } else {
                    let v = bus.read(self.addr);
                    self.exec_read(self.op, v);
                    Flow::Done
                }
            }
            MicroOp::ReadExec => {
                let v = bus.read(self.addr);
                self.exec_read(self.op, v);
                Flow::Done
            }
            // --- E05b : écriture et RMW (1 accès bus par micro-op) ---
            MicroOp::DummyReadIndexed => {
                let _ = bus.read((self.base & 0xFF00) | (self.addr & 0xFF));
                Flow::Next
            }
            MicroOp::WriteExec => {
                bus.write(self.addr, self.exec_write(self.op));
                Flow::Done
            }
            MicroOp::RmwRead => {
                self.data = bus.read(self.addr);
                Flow::Next
            }
            MicroOp::RmwDummyWrite => {
                let old = self.data;
                bus.write(self.addr, old); // ancienne valeur
                self.data = self.exec_rmw(self.op, old);
                Flow::Next
            }
            MicroOp::RmwWrite => {
                bus.write(self.addr, self.data);
                Flow::Done
            }
            // --- E09a : branchements et JMP (1 accès bus par micro-op) ---
            MicroOp::BranchFetch => {
                self.data = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                if !self.branch_taken() {
                    Flow::Done
                } else {
                    Flow::Next
                }
            }
            MicroOp::BranchTaken => {
                let _ = bus.read(self.pc); // lecture factice R*(PC)
                self.addr = self.pc.wrapping_add((self.data as i8) as u16);
                if (self.addr ^ self.pc) & 0xFF00 == 0 {
                    self.pc = self.addr;
                    Flow::Done
                } else {
                    Flow::Next // BranchFixPage suit
                }
            }
            MicroOp::BranchFixPage => {
                let _ = bus.read((self.pc & 0xFF00) | (self.addr & 0xFF));
                self.pc = self.addr;
                Flow::Done
            }
            MicroOp::JmpAbsHi => {
                let hi = bus.read(self.pc);
                self.pc = (u16::from(hi) << 8) | self.addr;
                Flow::Done
            }
            MicroOp::ReadIndirectLo => {
                self.data = bus.read(self.addr);
                Flow::Next
            }
            MicroOp::JmpIndirectHi => {
                // Bogue de page : l'octet haut est lu en (addr + 1) & $FFFF, pas addr + 1.
                let hi = bus.read((self.addr & 0xFF00) | ((self.addr.wrapping_add(1)) & 0xFF));
                self.pc = (u16::from(hi) << 8) | u16::from(self.data);
                Flow::Done
            }
            // --- E09b : pile et JSR/RTS (1 accès bus par micro-op) ---
            MicroOp::DummyReadStack => {
                let _ = bus.read(0x0100 + u16::from(self.s));
                Flow::Next
            }
            MicroOp::PushPch => {
                bus.write(0x0100 + u16::from(self.s), (self.pc >> 8) as u8);
                self.s = self.s.wrapping_sub(1);
                Flow::Next
            }
            MicroOp::PushPcl => {
                bus.write(0x0100 + u16::from(self.s), self.pc as u8);
                self.s = self.s.wrapping_sub(1);
                Flow::Next
            }
            MicroOp::PushA => {
                bus.write(0x0100 + u16::from(self.s), self.a);
                self.s = self.s.wrapping_sub(1);
                Flow::Done
            }
            MicroOp::PushPPhp => {
                // B forcé à 1 dans l'octet empilé (bit 5).
                bus.write(0x0100 + u16::from(self.s), self.p | 0x30);
                self.s = self.s.wrapping_sub(1);
                Flow::Done
            }
            MicroOp::PullA => {
                self.s = self.s.wrapping_add(1);
                let v = bus.read(0x0100 + u16::from(self.s));
                self.a = v;
                self.set_zn(v); // N, Z
                Flow::Done
            }
            MicroOp::PullPPlp => {
                self.s = self.s.wrapping_add(1);
                let v = bus.read(0x0100 + u16::from(self.s));
                self.p = (v & 0xCF) | 0x20; // B ignoré, U toujours posé
                Flow::Done
            }
            MicroOp::PullPcl => {
                self.s = self.s.wrapping_add(1);
                self.addr = u16::from(bus.read(0x0100 + u16::from(self.s)));
                Flow::Next
            }
            MicroOp::PullPchRts => {
                self.s = self.s.wrapping_add(1);
                let hi = bus.read(0x0100 + u16::from(self.s));
                self.pc = (u16::from(hi) << 8) | self.addr;
                Flow::Next // RtsIncPc suit
            }
            MicroOp::RtsIncPc => {
                let _ = bus.read(self.pc); // lecture factice R*(PC)
                self.pc = self.pc.wrapping_add(1);
                Flow::Done
            }
            MicroOp::JsrFetchHi => {
                let hi = bus.read(self.pc);
                self.pc = (u16::from(hi) << 8) | self.addr;
                Flow::Done
            }
            other => unimplemented!("{:?}", other),
        }
    }

    /// FetchAbsHiAddX / AddY : hi = R(PC++) ; base = hi:addr ; addr = base + reg ; crossed.
    fn fetch_abs_hi_add(&mut self, reg: u8, bus: &mut impl CpuBus) -> Flow {
        let hi = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let base = (u16::from(hi) << 8) | self.addr;
        self.base = base;
        self.addr = base.wrapping_add(u16::from(reg));
        self.crossed = (base ^ self.addr) & 0xFF00 != 0;
        Flow::Next
    }

    /// Condition d'un branchement (E09a) : vrai = branche prise.
    pub(crate) fn branch_taken(&self) -> bool {
        match self.op {
            Operation::Bpl => !self.flag(FLAG_N), // N = 0
            Operation::Bmi => self.flag(FLAG_N),  // N = 1
            Operation::Bvc => !self.flag(FLAG_V), // V = 0
            Operation::Bvs => self.flag(FLAG_V),  // V = 1
            Operation::Bcc => !self.flag(FLAG_C), // C = 0
            Operation::Bcs => self.flag(FLAG_C),  // C = 1
            Operation::Bne => !self.flag(FLAG_Z), // Z = 0
            Operation::Beq => self.flag(FLAG_Z),  // Z = 1
            other => unimplemented!("{:?}", other),
        }
    }

    /// « Quoi » d'une instruction implicite (E04b : NOP ; E06a : transferts ; E07b : INX/INY/DEX/DEY ; E08a : ASL/LSR/ROL/ROR A ; E09b : CLC SEC CLI SEI CLV CLD SED).
    pub(crate) fn exec_implied(&mut self, op: Operation) {
        match op {
            Operation::Nop => {}
            Operation::Tax => {
                self.x = self.a;
                self.set_zn(self.x); // N, Z
            }
            Operation::Tay => {
                self.y = self.a;
                self.set_zn(self.y); // N, Z
            }
            Operation::Txa => {
                self.a = self.x;
                self.set_zn(self.a); // N, Z
            }
            Operation::Tya => {
                self.a = self.y;
                self.set_zn(self.a); // N, Z
            }
            Operation::Tsx => {
                self.x = self.s;
                self.set_zn(self.x); // N, Z
            }
            Operation::Txs => self.s = self.x, // aucun flag
            Operation::Inx => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x); // N, Z
            }
            Operation::Iny => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y); // N, Z
            }
            Operation::Dex => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x); // N, Z
            }
            Operation::Dey => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y); // N, Z
            }
            Operation::Asl => self.a = self.asl(self.a), // E08a : mode accumulateur
            Operation::Lsr => self.a = self.lsr(self.a),
            Operation::Rol => self.a = self.rol(self.a),
            Operation::Ror => self.a = self.ror(self.a),
            Operation::Clc => self.set_flag(FLAG_C, false), // E09b : instructions de flags
            Operation::Sec => self.set_flag(FLAG_C, true),
            Operation::Cli => self.set_flag(FLAG_I, false),
            Operation::Sei => self.set_flag(FLAG_I, true),
            Operation::Clv => self.set_flag(FLAG_V, false),
            Operation::Cld => self.set_flag(FLAG_D, false),
            Operation::Sed => self.set_flag(FLAG_D, true),
            other => unimplemented!("{:?}", other),
        }
    }

    /// « Quoi » d'une lecture (E05a : LDA ; E06a : LDX/LDY ; E07a : ADC/SBC/AND/ORA/EOR ; E07b : CMP/CPX/CPY/BIT).
    pub(crate) fn exec_read(&mut self, op: Operation, value: u8) {
        match op {
            Operation::Lda => {
                self.a = value;
                self.set_zn(value); // N, Z
            }
            Operation::Ldx => {
                self.x = value;
                self.set_zn(value); // N, Z
            }
            Operation::Ldy => {
                self.y = value;
                self.set_zn(value); // N, Z
            }
            Operation::Adc => self.add_with_carry(value),
            Operation::Sbc => self.add_with_carry(value ^ 0xFF), // SBC = ADC avec M ^ 0xFF (D ignoré)
            Operation::And => {
                self.a &= value;
                self.set_zn(self.a); // N, Z
            }
            Operation::Ora => {
                self.a |= value;
                self.set_zn(self.a); // N, Z
            }
            Operation::Eor => {
                self.a ^= value;
                self.set_zn(self.a); // N, Z
            }
            Operation::Cmp => self.compare(self.a, value),
            Operation::Cpx => self.compare(self.x, value),
            Operation::Cpy => self.compare(self.y, value),
            Operation::Bit => {
                // A inchangé ; C non touché.
                self.set_flag(FLAG_Z, (self.a & value) == 0);
                self.set_flag(FLAG_N, value & 0x80 != 0);
                self.set_flag(FLAG_V, value & 0x40 != 0);
            }
            other => unimplemented!("{:?}", other),
        }
    }

    /// Valeur écrite par l'instruction (E05b : STA ; E06a : STX/STY).
    pub(crate) fn exec_write(&mut self, op: Operation) -> u8 {
        match op {
            Operation::Sta => self.a, // aucun flag
            Operation::Stx => self.x, // aucun flag
            Operation::Sty => self.y, // aucun flag
            other => unimplemented!("{:?}", other),
        }
    }

    /// Transformation lecture-modification-écriture (E05b : sonde INC ; E08b : ASL/LSR/ROL/ROR/DEC).
    pub(crate) fn exec_rmw(&mut self, op: Operation, data: u8) -> u8 {
        match op {
            Operation::Asl => self.asl(data),
            Operation::Lsr => self.lsr(data),
            Operation::Rol => self.rol(data),
            Operation::Ror => self.ror(data),
            Operation::Inc => {
                let v = data.wrapping_add(1);
                self.set_zn(v); // N, Z
                v
            }
            Operation::Dec => {
                let v = data.wrapping_sub(1);
                self.set_zn(v); // N, Z
                v
            }
            other => unimplemented!("{:?}", other),
        }
    }

    // ---------- Arithmétique (E07a) ----------

    /// ADC/SBC : sum = A + m + C (16 bits). Pose C, V, N, Z ; A = octet bas de sum.
    /// Réutilisée par RRA et ISB (E11).
    pub(crate) fn add_with_carry(&mut self, m: u8) {
        let cin = if self.flag(FLAG_C) { 1 } else { 0 };
        let sum = u16::from(self.a)
            .wrapping_add(u16::from(m))
            .wrapping_add(cin);
        self.set_flag(FLAG_C, sum > 0xFF);
        self.set_flag(
            FLAG_V,
            ((u16::from(self.a) ^ sum) & (u16::from(m) ^ sum) & 0x80) != 0,
        );
        let a = sum as u8;
        self.a = a;
        self.set_zn(a); // N, Z
    }

    // ---------- Comparaisons (E07b) ----------

    /// CMP/CPX/CPY : r = reg - M (wrapping) ; C = reg >= M ; Z = reg == M ; N = bit 7 de r.
    pub(crate) fn compare(&mut self, reg: u8, m: u8) {
        let r = reg.wrapping_sub(m);
        self.set_flag(FLAG_C, reg >= m);
        self.set_zn(r); // N, Z (r == 0 ⇔ reg == M)
    }

    // ---------- Décalages/rotations (E08a) ----------

    /// ASL : C = bit 7 de v ; r = v << 1 ; N, Z sur r.
    pub(crate) fn asl(&mut self, v: u8) -> u8 {
        self.set_flag(FLAG_C, v & 0x80 != 0);
        let r = v << 1;
        self.set_zn(r);
        r
    }

    /// LSR : C = bit 0 de v ; r = v >> 1 (N = 0).
    pub(crate) fn lsr(&mut self, v: u8) -> u8 {
        self.set_flag(FLAG_C, v & 0x01 != 0);
        let r = v >> 1;
        self.set_zn(r); // N = 0
        r
    }

    /// ROL : C' = bit 7 de v ; r = (v << 1) | C ; N, Z sur r.
    pub(crate) fn rol(&mut self, v: u8) -> u8 {
        let c = self.flag(FLAG_C) as u8;
        self.set_flag(FLAG_C, v & 0x80 != 0);
        let r = (v << 1) | c;
        self.set_zn(r);
        r
    }

    /// ROR : C' = bit 0 de v ; r = (v >> 1) | (C << 7).
    pub(crate) fn ror(&mut self, v: u8) -> u8 {
        let c = self.flag(FLAG_C) as u8;
        self.set_flag(FLAG_C, v & 0x01 != 0);
        let r = (v >> 1) | (c << 7);
        self.set_zn(r);
        r
    }

    // ---------- Aides sur les flags ----------

    /// Teste un bit de P.
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
    fn sondes_disponibles() {
        // Sondes E05a/E05b : LDA (lecture), STA (écriture), INC (RMW).
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
