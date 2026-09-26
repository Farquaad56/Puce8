// FICHIER GÉNÉRÉ — ne pas modifier à la main.
// Contrat : Docs_Implementation/annexes/D_micro_ops.md (section D2).
// Une MicroOp = UN cycle CPU = EXACTEMENT UN accès bus.

/// Résultat d'une micro-op : continuer la séquence, ou terminer l'instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Next,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicroOp {
    // --- communes
    DummyReadPc,     // R*(PC)
    DummyReadPcExec, // R*(PC) ; exec_implied(op) ; Done
    ReadImmExec,     // v = R(PC++) ; exec_read(op, v) ; Done
    // --- adressage
    FetchZp,              // addr = R(PC++)
    DummyReadZpAddX,      // R*(addr) ; addr = (addr + X) & 0xFF
    DummyReadZpAddY,      // R*(addr) ; addr = (addr + Y) & 0xFF
    FetchAbsLo,           // addr = R(PC++)
    FetchAbsHi,           // addr |= R(PC++) << 8
    FetchAbsHiAddX,       // hi = R(PC++) ; base = hi:lo ; addr = base + X ; crossed
    FetchAbsHiAddY,       // idem avec Y
    FetchPtr,             // ptr = R(PC++)
    DummyReadPtrAddX,     // R*(ptr) ; ptr = ptr + X (u8)
    ReadPtrLo,            // addr = R(ptr)
    ReadPtrHi,            // addr |= R((ptr + 1) & 0xFF) << 8
    ReadPtrHiAddY,        // hi = R((ptr+1)&0xFF) ; base = hi:lo ; addr = base + Y ; crossed
    ReadIndexedPageCheck, // crossed ? R*(non corrigée) : (v = R(addr) ; exec_read ; Done)
    DummyReadIndexed,     // R*(non corrigée), toujours (Write / Rmw)
    // --- données
    ReadExec,      // v = R(addr) ; exec_read(op, v) ; Done
    WriteExec,     // W(addr, exec_write(op)) ; Done
    RmwRead,       // data = R(addr)
    RmwDummyWrite, // W(addr, data) ; data = exec_rmw(op, data)
    RmwWrite,      // W(addr, data) ; Done
    // --- pile
    DummyReadStack,    // R*(0x0100 + S)
    DummyReadStackDec, // R*(0x0100 + S) ; S -= 1   (reset uniquement)
    PushPch,           // W(0x0100 + S, PC >> 8) ; S -= 1
    PushPcl,           // W(0x0100 + S, PC as u8) ; S -= 1
    PushA,             // W(0x0100 + S, A) ; S -= 1 ; Done
    PushPPhp,          // W(0x0100 + S, P | 0x30) ; S -= 1 ; Done
    PushPBrk, // W(0x0100 + S, P | 0x30) ; S -= 1 ; I = 1 ; vector = need_nmi ? $FFFA : $FFFE
    PushPInterrupt, // W(0x0100 + S, (P & !0x10) | 0x20) ; S -= 1 ; I = 1 ; vector = need_nmi ? $FFFA : $FFFE
    PullA,          // S += 1 ; A = R(0x0100 + S) ; N, Z ; Done
    PullPPlp,       // S += 1 ; P = (R(..) & 0xCF) | 0x20 ; Done
    PullP,          // S += 1 ; P = (R(..) & 0xCF) | 0x20   (RTI)
    PullPcl,        // S += 1 ; addr = R(..)
    PullPchRti,     // S += 1 ; PC = R(..):addr ; Done
    PullPchRts,     // S += 1 ; PC = R(..):addr
    RtsIncPc,       // R*(PC) ; PC += 1 ; Done
    // --- sauts
    JsrFetchHi,     // hi = R(PC) ; PC = hi:addr ; Done
    JmpAbsHi,       // hi = R(PC) ; PC = hi:addr ; Done
    ReadIndirectLo, // data = R(addr)
    JmpIndirectHi,  // hi = R((addr & 0xFF00) | ((addr + 1) & 0xFF)) ; PC = hi:data ; Done
    // --- interruptions
    BrkPadding,   // R(PC++) (octet ignoré)
    ReadVectorLo, // addr = R(vector)
    ReadVectorHi, // PC = R(vector + 1):addr ; si vector == $FFFA : need_nmi = false ; Done
    // --- branchements
    BranchFetch,   // data = R(PC++) ; condition fausse → Done
    BranchTaken,   // R*(PC) ; addr = PC + (data as i8) ; même page → PC = addr ; Done
    BranchFixPage, // R*((PC & 0xFF00) | (addr & 0xFF)) ; PC = addr ; Done
    // --- blocage
    Jam, // R*(0xFFFF) ; jammed = true ; Done
}

impl MicroOp {
    /// Vrai si cette micro-op ÉCRIT sur le bus (la DMA ne peut pas arrêter le CPU sur une écriture).
    pub const fn is_write(self) -> bool {
        matches!(
            self,
            MicroOp::WriteExec
                | MicroOp::RmwDummyWrite
                | MicroOp::RmwWrite
                | MicroOp::PushPch
                | MicroOp::PushPcl
                | MicroOp::PushA
                | MicroOp::PushPPhp
                | MicroOp::PushPBrk
                | MicroOp::PushPInterrupt
        )
    }
}
