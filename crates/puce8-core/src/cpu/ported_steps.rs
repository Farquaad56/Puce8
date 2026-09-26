// FICHIER GÉNÉRÉ — ne pas modifier à la main.
// Séquences cycle par cycle portées de la référence 6502 (annexe D3/D4).
// Le cycle 1 (lecture de l'opcode) n'est PAS dans les séquences : Cpu::tick le fait.

use super::micro_op::MicroOp::{self, *};
use super::opcodes::{Mode, OPCODES};
use super::operations::{class, Class};

// ---------- Read ----------
const IMP: &[MicroOp] = &[DummyReadPcExec];
const IMM: &[MicroOp] = &[ReadImmExec];
const ZP0_R: &[MicroOp] = &[FetchZp, ReadExec];
const ZPX_R: &[MicroOp] = &[FetchZp, DummyReadZpAddX, ReadExec];
const ZPY_R: &[MicroOp] = &[FetchZp, DummyReadZpAddY, ReadExec];
const ABS_R: &[MicroOp] = &[FetchAbsLo, FetchAbsHi, ReadExec];
const ABX_R: &[MicroOp] = &[FetchAbsLo, FetchAbsHiAddX, ReadIndexedPageCheck, ReadExec];
const ABY_R: &[MicroOp] = &[FetchAbsLo, FetchAbsHiAddY, ReadIndexedPageCheck, ReadExec];
const IZX_R: &[MicroOp] = &[FetchPtr, DummyReadPtrAddX, ReadPtrLo, ReadPtrHi, ReadExec];
const IZY_R: &[MicroOp] = &[
    FetchPtr,
    ReadPtrLo,
    ReadPtrHiAddY,
    ReadIndexedPageCheck,
    ReadExec,
];
// ---------- Write ----------
const ZP0_W: &[MicroOp] = &[FetchZp, WriteExec];
const ZPX_W: &[MicroOp] = &[FetchZp, DummyReadZpAddX, WriteExec];
const ZPY_W: &[MicroOp] = &[FetchZp, DummyReadZpAddY, WriteExec];
const ABS_W: &[MicroOp] = &[FetchAbsLo, FetchAbsHi, WriteExec];
const ABX_W: &[MicroOp] = &[FetchAbsLo, FetchAbsHiAddX, DummyReadIndexed, WriteExec];
const ABY_W: &[MicroOp] = &[FetchAbsLo, FetchAbsHiAddY, DummyReadIndexed, WriteExec];
const IZX_W: &[MicroOp] = &[FetchPtr, DummyReadPtrAddX, ReadPtrLo, ReadPtrHi, WriteExec];
const IZY_W: &[MicroOp] = &[
    FetchPtr,
    ReadPtrLo,
    ReadPtrHiAddY,
    DummyReadIndexed,
    WriteExec,
];
// ---------- Rmw ----------
const ZP0_M: &[MicroOp] = &[FetchZp, RmwRead, RmwDummyWrite, RmwWrite];
const ZPX_M: &[MicroOp] = &[FetchZp, DummyReadZpAddX, RmwRead, RmwDummyWrite, RmwWrite];
const ABS_M: &[MicroOp] = &[FetchAbsLo, FetchAbsHi, RmwRead, RmwDummyWrite, RmwWrite];
const ABX_M: &[MicroOp] = &[
    FetchAbsLo,
    FetchAbsHiAddX,
    DummyReadIndexed,
    RmwRead,
    RmwDummyWrite,
    RmwWrite,
];
const ABY_M: &[MicroOp] = &[
    FetchAbsLo,
    FetchAbsHiAddY,
    DummyReadIndexed,
    RmwRead,
    RmwDummyWrite,
    RmwWrite,
];
const IZX_M: &[MicroOp] = &[
    FetchPtr,
    DummyReadPtrAddX,
    ReadPtrLo,
    ReadPtrHi,
    RmwRead,
    RmwDummyWrite,
    RmwWrite,
];
const IZY_M: &[MicroOp] = &[
    FetchPtr,
    ReadPtrLo,
    ReadPtrHiAddY,
    DummyReadIndexed,
    RmwRead,
    RmwDummyWrite,
    RmwWrite,
];
// ---------- Spéciales (annexe D4) ----------
const BRANCH: &[MicroOp] = &[BranchFetch, BranchTaken, BranchFixPage];
const JMP_ABS: &[MicroOp] = &[FetchAbsLo, JmpAbsHi];
const JMP_IND: &[MicroOp] = &[FetchAbsLo, FetchAbsHi, ReadIndirectLo, JmpIndirectHi];
const JSR: &[MicroOp] = &[FetchAbsLo, DummyReadStack, PushPch, PushPcl, JsrFetchHi];
const RTS: &[MicroOp] = &[DummyReadPc, DummyReadStack, PullPcl, PullPchRts, RtsIncPc];
const RTI: &[MicroOp] = &[DummyReadPc, DummyReadStack, PullP, PullPcl, PullPchRti];
const PHA: &[MicroOp] = &[DummyReadPc, PushA];
const PHP: &[MicroOp] = &[DummyReadPc, PushPPhp];
const PLA: &[MicroOp] = &[DummyReadPc, DummyReadStack, PullA];
const PLP: &[MicroOp] = &[DummyReadPc, DummyReadStack, PullPPlp];
const BRK: &[MicroOp] = &[
    BrkPadding,
    PushPch,
    PushPcl,
    PushPBrk,
    ReadVectorLo,
    ReadVectorHi,
];
const JAM: &[MicroOp] = &[Jam];

/// Séquence de reset (7 cycles, TOUS dans la liste ; vector = $FFFC).
pub const RESET_SEQ: &[MicroOp] = &[
    DummyReadPc,
    DummyReadPc,
    DummyReadStackDec,
    DummyReadStackDec,
    DummyReadStackDec,
    ReadVectorLo,
    ReadVectorHi,
];
/// Séquence d'interruption NMI/IRQ : 6 micro-ops. Le 1er cycle (R*(PC) à la place de la lecture
/// de l'opcode) est fait par Cpu::begin_interrupt. Total : 7 cycles.
pub const INTERRUPT_SEQ: &[MicroOp] = &[
    DummyReadPc,
    PushPch,
    PushPcl,
    PushPInterrupt,
    ReadVectorLo,
    ReadVectorHi,
];

/// Séquence de micro-ops d'un opcode (sans le cycle de lecture de l'opcode).
pub fn ported_steps(opcode: u8) -> &'static [MicroOp] {
    let mode = OPCODES[opcode as usize].mode;
    match class(opcode) {
        Class::Implied => IMP,
        Class::Read => match mode {
            Mode::Imm => IMM,
            Mode::Zp0 => ZP0_R,
            Mode::Zpx => ZPX_R,
            Mode::Zpy => ZPY_R,
            Mode::Abs => ABS_R,
            Mode::Abx => ABX_R,
            Mode::Aby => ABY_R,
            Mode::Izx => IZX_R,
            Mode::Izy => IZY_R,
            _ => unreachable!("Read en mode {:?} (opcode {:02X})", mode, opcode),
        },
        Class::Write => match mode {
            Mode::Zp0 => ZP0_W,
            Mode::Zpx => ZPX_W,
            Mode::Zpy => ZPY_W,
            Mode::Abs => ABS_W,
            Mode::Abx => ABX_W,
            Mode::Aby => ABY_W,
            Mode::Izx => IZX_W,
            Mode::Izy => IZY_W,
            _ => unreachable!("Write en mode {:?} (opcode {:02X})", mode, opcode),
        },
        Class::Rmw => match mode {
            Mode::Zp0 => ZP0_M,
            Mode::Zpx => ZPX_M,
            Mode::Abs => ABS_M,
            Mode::Abx => ABX_M,
            Mode::Aby => ABY_M,
            Mode::Izx => IZX_M,
            Mode::Izy => IZY_M,
            _ => unreachable!("Rmw en mode {:?} (opcode {:02X})", mode, opcode),
        },
        Class::Special => match opcode {
            0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0 => BRANCH,
            0x4C => JMP_ABS,
            0x6C => JMP_IND,
            0x20 => JSR,
            0x60 => RTS,
            0x40 => RTI,
            0x48 => PHA,
            0x08 => PHP,
            0x68 => PLA,
            0x28 => PLP,
            0x00 => BRK,
            _ => JAM, // les 12 opcodes JAM
        },
    }
}
