// FICHIER GÉNÉRÉ — ne pas modifier à la main.
// Source : Docs_Implementation/annexes/code/ (vérifié contre nestest.log).

/// Une opération = le « quoi » d'une instruction (l'adressage est dans ported_steps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Adc,
    Ahx,
    Alr,
    Anc,
    And,
    Arr,
    Asl,
    Axs,
    Bcc,
    Bcs,
    Beq,
    Bit,
    Bmi,
    Bne,
    Bpl,
    Brk,
    Bvc,
    Bvs,
    Clc,
    Cld,
    Cli,
    Clv,
    Cmp,
    Cpx,
    Cpy,
    Dcp,
    Dec,
    Dex,
    Dey,
    Eor,
    Inc,
    Inx,
    Iny,
    Isb,
    Jam,
    Jmp,
    Jsr,
    Las,
    Lax,
    Lda,
    Ldx,
    Ldy,
    Lsr,
    Lxa,
    Nop,
    Ora,
    Pha,
    Php,
    Pla,
    Plp,
    Rla,
    Rol,
    Ror,
    Rra,
    Rti,
    Rts,
    Sax,
    Sbc,
    Sec,
    Sed,
    Sei,
    Shx,
    Shy,
    Slo,
    Sre,
    Sta,
    Stx,
    Sty,
    Tas,
    Tax,
    Tay,
    Tsx,
    Txa,
    Txs,
    Tya,
    Xaa,
}

/// Classe d'accès mémoire (annexe D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Read,
    Write,
    Rmw,
    Implied,
    Special,
}

use Operation::*;

pub static OPERATION_TABLE: [Operation; 256] = [
    Brk, Ora, Jam, Slo, Nop, Ora, Asl, Slo, Php, Ora, Asl, Anc, Nop, Ora, Asl, Slo, // 0x
    Bpl, Ora, Jam, Slo, Nop, Ora, Asl, Slo, Clc, Ora, Nop, Slo, Nop, Ora, Asl, Slo, // 1x
    Jsr, And, Jam, Rla, Bit, And, Rol, Rla, Plp, And, Rol, Anc, Bit, And, Rol, Rla, // 2x
    Bmi, And, Jam, Rla, Nop, And, Rol, Rla, Sec, And, Nop, Rla, Nop, And, Rol, Rla, // 3x
    Rti, Eor, Jam, Sre, Nop, Eor, Lsr, Sre, Pha, Eor, Lsr, Alr, Jmp, Eor, Lsr, Sre, // 4x
    Bvc, Eor, Jam, Sre, Nop, Eor, Lsr, Sre, Cli, Eor, Nop, Sre, Nop, Eor, Lsr, Sre, // 5x
    Rts, Adc, Jam, Rra, Nop, Adc, Ror, Rra, Pla, Adc, Ror, Arr, Jmp, Adc, Ror, Rra, // 6x
    Bvs, Adc, Jam, Rra, Nop, Adc, Ror, Rra, Sei, Adc, Nop, Rra, Nop, Adc, Ror, Rra, // 7x
    Nop, Sta, Nop, Sax, Sty, Sta, Stx, Sax, Dey, Nop, Txa, Xaa, Sty, Sta, Stx, Sax, // 8x
    Bcc, Sta, Jam, Ahx, Sty, Sta, Stx, Sax, Tya, Sta, Txs, Tas, Shy, Sta, Shx, Ahx, // 9x
    Ldy, Lda, Ldx, Lax, Ldy, Lda, Ldx, Lax, Tay, Lda, Tax, Lxa, Ldy, Lda, Ldx, Lax, // Ax
    Bcs, Lda, Jam, Lax, Ldy, Lda, Ldx, Lax, Clv, Lda, Tsx, Las, Ldy, Lda, Ldx, Lax, // Bx
    Cpy, Cmp, Nop, Dcp, Cpy, Cmp, Dec, Dcp, Iny, Cmp, Dex, Axs, Cpy, Cmp, Dec, Dcp, // Cx
    Bne, Cmp, Jam, Dcp, Nop, Cmp, Dec, Dcp, Cld, Cmp, Nop, Dcp, Nop, Cmp, Dec, Dcp, // Dx
    Cpx, Sbc, Nop, Isb, Cpx, Sbc, Inc, Isb, Inx, Sbc, Nop, Sbc, Cpx, Sbc, Inc, Isb, // Ex
    Beq, Sbc, Jam, Isb, Nop, Sbc, Inc, Isb, Sed, Sbc, Nop, Isb, Nop, Sbc, Inc, Isb, // Fx
];

pub static CLASS_TABLE: [Class; 256] = {
    use Class::*;
    [
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Special, Read, Implied, Read, Read,
        Read, Rmw, Rmw, // 0x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Rmw, Read, Read,
        Rmw, Rmw, // 1x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Special, Read, Implied, Read, Read,
        Read, Rmw, Rmw, // 2x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Rmw, Read, Read,
        Rmw, Rmw, // 3x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Special, Read, Implied, Read, Special,
        Read, Rmw, Rmw, // 4x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Rmw, Read, Read,
        Rmw, Rmw, // 5x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Special, Read, Implied, Read, Special,
        Read, Rmw, Rmw, // 6x
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Rmw, Read, Read,
        Rmw, Rmw, // 7x
        Read, Write, Read, Write, Write, Write, Write, Write, Implied, Read, Implied, Read, Write,
        Write, Write, Write, // 8x
        Special, Write, Special, Write, Write, Write, Write, Write, Implied, Write, Implied, Write,
        Write, Write, Write, Write, // 9x
        Read, Read, Read, Read, Read, Read, Read, Read, Implied, Read, Implied, Read, Read, Read,
        Read, Read, // Ax
        Special, Read, Special, Read, Read, Read, Read, Read, Implied, Read, Implied, Read, Read,
        Read, Read, Read, // Bx
        Read, Read, Read, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Read, Read, Read, Rmw,
        Rmw, // Cx
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Rmw, Read, Read,
        Rmw, Rmw, // Dx
        Read, Read, Read, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Read, Read, Read, Rmw,
        Rmw, // Ex
        Special, Read, Special, Rmw, Read, Read, Rmw, Rmw, Implied, Read, Implied, Rmw, Read, Read,
        Rmw, Rmw, // Fx
    ]
};

#[inline]
pub fn operation(opcode: u8) -> Operation {
    OPERATION_TABLE[opcode as usize]
}

#[inline]
pub fn class(opcode: u8) -> Class {
    CLASS_TABLE[opcode as usize]
}
