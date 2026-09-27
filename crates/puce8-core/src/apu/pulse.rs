//! Canal pulse (E31b1) : timer 11 bits (avance tous les 2 cycles CPU), duty, enveloppe, sweep.
//! Le compteur de longueur est gere par `Apu` (sortie coupee quand il vaut 0).
// wiki: APU_Pulse

use super::envelope::Envelope;
use super::sweep::Sweep;

/// Sequences de duty (8 pas) : 12,5 %, 25 %, 50 %, 75 % (25 % inverse).
pub const DUTY: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

/// Canal pulse 1 ou 2.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    /// true = pulse 1 (negation du sweep en complement a un).
    pub is_pulse1: bool,
    /// DD : duty (0-3).
    pub duty: u8,
    /// Pas courant du sequenceur (0-7).
    pub step: u8,
    /// Periode du timer (11 bits).
    pub period: u16,
    timer: u16,
    pub envelope: Envelope,
    pub sweep: Sweep,
}

impl Pulse {
    pub fn new(is_pulse1: bool) -> Self {
        Pulse {
            is_pulse1,
            ..Default::default()
        }
    }

    /// Ecriture d'un registre du canal (`addr & 3`) : `DDLC VVVV`, sweep, timer bas, `LLLL LTTT`.
    /// Le chargement de la longueur (bits 7-3 du 4e registre) est fait par `Apu`.
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr & 3 {
            0 => {
                self.duty = v >> 6;
                self.envelope.write(v);
            }
            1 => self.sweep.write(v),
            2 => self.period = (self.period & 0x0700) | u16::from(v),
            _ => {
                self.period = (self.period & 0x00FF) | (u16::from(v & 7) << 8);
                self.step = 0; // sequenceur remis a 0
                self.envelope.start = true;
            }
        }
    }

    /// Un pas du timer (appele par `Apu` tous les 2 cycles CPU).
    pub fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.step = (self.step + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    /// Horloge "demi" : sweep.
    pub fn clock_half(&mut self) {
        self.sweep.clock(&mut self.period, self.is_pulse1);
    }

    /// Sortie 0-15 (hors compteur de longueur) : 0 si muet ou bit de duty a 0, sinon volume.
    pub fn output(&self) -> u8 {
        let bit = DUTY[usize::from(self.duty & 3)][usize::from(self.step & 7)];
        if bit == 0 || self.sweep.muted(self.period, self.is_pulse1) {
            0
        } else {
            self.envelope.output()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frequence NTSC du CPU (Hz).
    const CPU_HZ: u32 = 1_789_773;

    /// Pulse duty `duty`, volume constant 15, periode `t`.
    fn pulse(duty: u8, t: u16) -> Pulse {
        let mut p = Pulse::new(false);
        p.write(0x4000, (duty << 6) | 0x30 | 15);
        p.write(0x4002, (t & 0xFF) as u8);
        p.write(0x4003, (t >> 8) as u8);
        p
    }

    /// Simule `cycles` cycles CPU : (frequence en Hz par les fronts montants, crete, part du temps a 1).
    fn mesurer(p: &mut Pulse, cycles: u32) -> (f64, u8, f64) {
        let (mut fronts, mut crete, mut haut, mut prec) = (0u32, 0u8, 0u32, 0u8);
        for c in 0..cycles {
            if c.is_multiple_of(2) {
                p.clock_timer();
            }
            let out = p.output();
            if prec == 0 && out > 0 {
                fronts += 1;
            }
            if out > 0 {
                haut += 1;
            }
            crete = crete.max(out);
            prec = out;
        }
        let secondes = f64::from(cycles) / f64::from(CPU_HZ);
        (
            f64::from(fronts) / secondes,
            crete,
            f64::from(haut) / f64::from(cycles),
        )
    }

    #[test]
    fn frequence_440() {
        let mut p = pulse(2, 253); // 1 789 773 / (16 x 254) = 440,4 Hz
        let (f, crete, _) = mesurer(&mut p, CPU_HZ);
        assert!((f - 440.4).abs() < 4.4, "f = {f}");
        assert_eq!(crete, 15);
    }

    #[test]
    fn duty_25() {
        let mut p = pulse(1, 253);
        let (_, _, part) = mesurer(&mut p, CPU_HZ / 10);
        assert!((part - 0.25).abs() < 0.01, "part = {part}");
    }

    #[test]
    fn muet_periode_basse() {
        let mut p = pulse(2, 7);
        let (_, crete, _) = mesurer(&mut p, 10_000);
        assert_eq!(crete, 0);
    }
}
