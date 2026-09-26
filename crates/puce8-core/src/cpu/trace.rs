//! Traceur au format nestest (E10a) : capture de l'état AVANT instruction + comparaison.

use super::opcodes::{Mode, OPCODES};
use super::{Cpu, CpuBus};

/// État capturé avant l'instruction située à `cpu.pc` (lu avec `peek`, sans effet).
#[derive(Clone, Copy, Debug)]
pub struct TraceState {
    pub pc: u16,
    /// Octets de l'instruction à PC (3 au maximum ; slots inutilisés = 0).
    pub bytes: [u8; 3],
    /// Longueur de l'instruction en octets.
    pub len: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
    pub s: u8,
    pub cycles: u64,
    /// PPU (ligne de frame, point du cycle) — rempli en E13.
    pub ppu: Option<(u16, u16)>,
}

/// Capture l'état avant l'instruction à `cpu.pc`.
pub fn capture(cpu: &Cpu, bus: &impl CpuBus) -> TraceState {
    // À la frontière d'instruction, l'opcode suivant n'est pas encore lu dans `cpu.opcode` : on le lit dans le bus.
    let op = bus.peek(cpu.pc);
    let len = 1 + OPCODES[op as usize].mode.operand_len();
    let mut bytes = [0u8; 3];
    for (i, slot) in bytes.iter_mut().enumerate() {
        if i >= len.min(3) as usize {
            break;
        }
        *slot = bus.peek(cpu.pc.wrapping_add(i as u16));
    }
    TraceState {
        pc: cpu.pc,
        bytes,
        len,
        a: cpu.a,
        x: cpu.x,
        y: cpu.y,
        p: cpu.p,
        s: cpu.s,
        cycles: cpu.cycles,
        ppu: None, // E13
    }
}

/// Formate l'état en ligne nestest (colonnes fixes ; le désassemblage n'est JAMAIS comparé).
pub fn format_nestest(t: &TraceState) -> String {
    let mut s = format!("{:04X}  ", t.pc);
    // Champ octets, largeur 8 (colonnes 6-13).
    let n = t.len.min(3) as usize;
    let mut bf = String::new();
    for i in 0..n {
        if i > 0 {
            bf.push(' ');
        }
        push_hex2(&mut bf, t.bytes[i]);
    }
    s.push_str(&format!("{:<8}  ", bf));
    // Champ désassemblage, largeur 32 (colonnes 16-47).
    let info = OPCODES[t.bytes[0] as usize];
    let mut d = String::new();
    if !info.official {
        d.push('*');
    }
    d.push_str(info.mnemonic);
    push_operand(&mut d, &info.mode, t);
    s.push_str(&format!("{:<32}", d));
    // Registres (colonne 48).
    s.push_str(&format!(
        "A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
        t.a, t.x, t.y, t.p, t.s
    ));
    match t.ppu {
        Some((line, point)) => s.push_str(&format!(
            " PPU:{:>7} CYC:{}",
            format!("{}, {}", line, point),
            t.cycles
        )),
        None => s.push_str(&format!(" CYC:{}", t.cycles)),
    }
    s
}

/// Compare l'état capturé à une ligne de référence nestest.
/// La ligne est parsée par recherche des étiquettes (`"A:"`, `"CYC:"`…), pas en position fixe au-delà de la colonne 16.
pub fn compare_with_reference(
    reference_line: &str,
    t: &TraceState,
    check_ppu: bool,
) -> Result<(), String> {
    // PC : colonnes 0-3.
    let pc_ref = u16::from_str_radix(&reference_line[0..4], 16)
        .map_err(|_| "mauvaise ligne de référence (PC)".to_string())?;
    if pc_ref != t.pc {
        return Err(format!(
            "champ PC : attendu {:04X}, obtenu {:04X}",
            pc_ref, t.pc
        ));
    }
    // Octets : colonnes 6-13.
    let mut ref_bytes = Vec::new();
    for tok in reference_line[6..14].split(' ') {
        if tok.is_empty() {
            break;
        }
        match u8::from_str_radix(tok, 16) {
            Ok(b) => ref_bytes.push(b),
            Err(_) => return Err(format!("mauvaise ligne de référence (octets) : {:?}", tok)),
        }
    }
    let n = t.len.min(3) as usize;
    for (i, (rb, tb)) in ref_bytes.iter().zip(t.bytes.iter()).take(n).enumerate() {
        if rb != tb {
            return Err(format!(
                "champ octets[{}] : attendu {:02X}, obtenu {:02X}",
                i, rb, tb
            ));
        }
    }
    if n != ref_bytes.len() {
        return Err(format!(
            "champ octets : attendu {} octet(s), obtenu {}",
            ref_bytes.len(),
            n
        ));
    }
    // Registres : recherche des étiquettes.
    for (label, got) in [("A", t.a), ("X", t.x), ("Y", t.y), ("P", t.p), ("SP", t.s)] {
        let exp = parse_reg(reference_line, label)
            .ok_or_else(|| format!("mauvaise ligne de référence : étiquette {} absente", label))?;
        if exp != got {
            return Err(format!(
                "champ {} : attendu {:02X}, obtenu {:02X}",
                label, exp, got
            ));
        }
    }
    // CYC.
    let cyc_ref = parse_dec(reference_line, "CYC")
        .ok_or_else(|| "mauvaise ligne de référence : étiquette CYC absente".to_string())?;
    if cyc_ref != t.cycles {
        return Err(format!(
            "champ CYC : attendu {}, obtenu {}",
            cyc_ref, t.cycles
        ));
    }
    // PPU (facultatif).
    if check_ppu {
        let ppu_ref = parse_ppu(reference_line)
            .ok_or_else(|| "mauvaise ligne de référence : étiquette PPU absente".to_string())?;
        match t.ppu {
            Some(got) if got == ppu_ref => {}
            _ => {
                return Err(format!(
                    "champ PPU : attendu {:?}, obtenu {:?}",
                    ppu_ref, t.ppu
                ))
            }
        }
    }
    Ok(())
}

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
];

fn push_hex2(s: &mut String, v: u8) {
    s.push(HEX[(v >> 4) as usize]);
    s.push(HEX[(v & 0x0F) as usize]);
}

fn push_hex(d: &mut String, prefix: &str, width: usize, v: u16) {
    d.push_str(prefix);
    for i in (0..width).rev() {
        d.push(HEX[((v >> (i * 4)) & 0x0F) as usize]);
    }
}

fn push_operand(d: &mut String, mode: &Mode, t: &TraceState) {
    let b0 = *t.bytes.get(1).unwrap_or(&0);
    let b1 = *t.bytes.get(2).unwrap_or(&0);
    if *mode != Mode::Imp {
        d.push(' ');
    }
    match *mode {
        Mode::Imp => {}
        Mode::Acc => d.push('A'),
        Mode::Imm => push_hex(d, "#$", 2, u16::from(b0)),
        Mode::Zp0 => push_hex(d, "$", 2, u16::from(b0)),
        Mode::Zpx => {
            push_hex(d, "$", 2, u16::from(b0));
            d.push_str(",X");
        }
        Mode::Zpy => {
            push_hex(d, "$", 2, u16::from(b0));
            d.push_str(",Y");
        }
        Mode::Abs => push_hex(d, "$", 4, (u16::from(b1) << 8) | u16::from(b0)),
        Mode::Abx => {
            push_hex(d, "$", 4, (u16::from(b1) << 8) | u16::from(b0));
            d.push_str(",X");
        }
        Mode::Aby => {
            push_hex(d, "$", 4, (u16::from(b1) << 8) | u16::from(b0));
            d.push_str(",Y");
        }
        Mode::Ind => {
            d.push('(');
            push_hex(d, "$", 4, (u16::from(b1) << 8) | u16::from(b0));
            d.push(')');
        }
        Mode::Izx => {
            d.push('(');
            push_hex(d, "$", 2, u16::from(b0));
            d.push_str(",X)");
        }
        Mode::Izy => {
            d.push('(');
            push_hex(d, "$", 2, u16::from(b0));
            d.push_str("),Y");
        }
        Mode::Rel => {
            let target = t.pc.wrapping_add(1).wrapping_add(b0 as i8 as u16);
            push_hex(d, "$", 4, target);
        }
    }
}

/// Valeur hexadécimale (2 chiffres) juste après l'étiquette `" <label>:"`.
fn parse_reg(line: &str, label: &str) -> Option<u8> {
    let i = line.find(format!(" {}:", label).as_str())?;
    let start = i + 1 + label.len() + 1;
    u8::from_str_radix(&line[start..start + 2], 16).ok()
}

/// Valeur décimale juste après l'étiquette `" <label>:"`.
fn parse_dec(line: &str, label: &str) -> Option<u64> {
    let i = line.find(format!(" {}:", label).as_str())?;
    let v = &line[i + 1 + label.len() + 1..];
    let digits: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// PPU : `" <label>:"` suivi de `ligne, point`.
fn parse_ppu(line: &str) -> Option<(u16, u16)> {
    let i = line.find(" PPU:")?;
    let v = &line[i + 5..];
    let (a, b) = v.split_once(',')?;
    Some((lead_digits(a)? as u16, lead_digits(b)? as u16))
}

fn lead_digits(s: &str) -> Option<u32> {
    let d: String = s
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if d.is_empty() {
        None
    } else {
        d.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::test_bus::TestBus;

    /// État de la ligne 1 du nestest.log.
    fn line1_state() -> TraceState {
        TraceState {
            pc: 0xC000,
            bytes: [0x4C, 0xF5, 0xC5],
            len: 3,
            a: 0x00,
            x: 0x00,
            y: 0x00,
            p: 0x24,
            s: 0xFD,
            cycles: 7,
            ppu: Some((0, 21)),
        }
    }

    const LIGNE1: &str = "C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 21 CYC:7";

    #[test]
    fn format_ligne1() {
        let s = format_nestest(&line1_state());
        assert!(s.starts_with("C000  4C F5 C5"));
        assert!(s.contains("P:24 SP:FD"));
        assert!(s.contains("CYC:7"));
        // Alignement exact sur la ligne réelle du nestest.log.
        assert_eq!(s, LIGNE1);
    }

    #[test]
    fn compare_ok() {
        assert!(compare_with_reference(LIGNE1, &line1_state(), true).is_ok());
    }

    #[test]
    fn compare_ko_p() {
        let mut t = line1_state();
        t.p = 0x25;
        let err = compare_with_reference(LIGNE1, &t, false).unwrap_err();
        assert!(err.contains("P"));
    }

    /// Ligne 2 du nestest.log (instruction de 2 octets) : alignement des colonnes.
    #[test]
    fn format_ligne2() {
        let t = TraceState {
            pc: 0xC5F5,
            bytes: [0xA2, 0x00, 0x00],
            len: 2,
            a: 0x00,
            x: 0x00,
            y: 0x00,
            p: 0x24,
            s: 0xFD,
            cycles: 10,
            ppu: Some((0, 30)),
        };
        assert_eq!(
            format_nestest(&t),
            "C5F5  A2 00     LDX #$00                        A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 30 CYC:10"
        );
    }

    #[test]
    fn capture_ligne1() {
        let mut bus = TestBus::new();
        bus.load(0xC000, &[0x4C, 0xF5, 0xC5]);
        let mut cpu = Cpu::new();
        cpu.pc = 0xC000;
        cpu.opcode = 0x4C; // JMP : 3 octets
        cpu.s = 0xFD;
        cpu.cycles = 7;
        let t = capture(&cpu, &bus);
        assert_eq!(t.pc, 0xC000);
        assert_eq!(t.bytes, [0x4C, 0xF5, 0xC5]);
        assert_eq!(t.len, 3);
        assert_eq!(t.a, 0x00);
        assert_eq!(t.x, 0x00);
        assert_eq!(t.y, 0x00);
        assert_eq!(t.p, 0x24);
        assert_eq!(t.s, 0xFD);
        assert_eq!(t.cycles, 7);
        assert_eq!(t.ppu, None);
    }

    #[test]
    fn capture_len_selon_mode() {
        let mut bus = TestBus::new();
        bus.load(0x8000, &[0xA2, 0x11]); // LDX #$11 : 2 octets
        let mut cpu = Cpu::new();
        cpu.pc = 0x8000;
        cpu.opcode = 0xA2;
        let t = capture(&cpu, &bus);
        assert_eq!(t.len, 2);
        assert_eq!(&t.bytes[..2], &[0xA2, 0x11]);

        bus.load(0x8000, &[0xEA]); // NOP : 1 octet
        cpu.opcode = 0xEA;
        let t = capture(&cpu, &bus);
        assert_eq!(t.len, 1);
    }
}
