//! Canal de bruit (E32b1) : LFSR 15 bits, periodes, enveloppe.
//! Le compteur de longueur est gere par `Apu` (sortie coupee quand il vaut 0).
// wiki: APU_Noise

use super::envelope::Envelope;

/// Periodes NTSC en cycles CPU ($400E bits 0-3).
pub const NOISE_PERIODS: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

/// Canal de bruit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Noise {
    pub envelope: Envelope,
    /// M : mode court (retroaction sur le bit 6).
    pub mode: bool,
    /// Periode en cycles CPU.
    pub period: u16,
    timer: u16,
    /// Registre a decalage de 15 bits (valeur initiale 1).
    pub lfsr: u16,
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

impl Noise {
    pub fn new() -> Self {
        Noise {
            envelope: Envelope::default(),
            mode: false,
            period: NOISE_PERIODS[0],
            timer: 0,
            lfsr: 1,
        }
    }

    /// Ecriture d'un registre (`addr & 3`) : $400C `--LC VVVV`, $400E `M--- PPPP`, $400F `LLLL L---`.
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr & 3 {
            0 => self.envelope.write(v),
            2 => {
                self.mode = v & 0x80 != 0;
                self.period = NOISE_PERIODS[usize::from(v & 0x0F)];
            }
            3 => self.envelope.start = true, // la longueur est chargee par `Apu`
            _ => {}                          // $400D : inutilise
        }
    }

    /// Un pas du LFSR : fb = bit0 ^ (bit6 si M, sinon bit1) ; decalage ; bit14 = fb.
    pub fn step_lfsr(&mut self) {
        let autre = if self.mode { 6 } else { 1 };
        let fb = (self.lfsr ^ (self.lfsr >> autre)) & 1;
        self.lfsr = (self.lfsr >> 1) | (fb << 14);
    }

    /// Un cycle CPU : le LFSR avance tous les `period` cycles.
    pub fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period.saturating_sub(1);
            self.step_lfsr();
        } else {
            self.timer -= 1;
        }
    }

    /// Sortie 0-15 (hors compteur de longueur) : 0 si le bit 0 du LFSR vaut 1, sinon volume.
    pub fn output(&self) -> u8 {
        if self.lfsr & 1 == 1 {
            0
        } else {
            self.envelope.output()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nombre de pas pour revenir a la valeur de depart (au plus `max`).
    fn periode_lfsr(n: &mut Noise, max: u32) -> Option<u32> {
        let depart = n.lfsr;
        (1..=max).find(|_| {
            n.step_lfsr();
            n.lfsr == depart
        })
    }

    #[test]
    fn lfsr_mode0() {
        let mut n = Noise::new();
        assert_eq!(periode_lfsr(&mut n, 40_000), Some(32_767));
    }

    #[test]
    fn lfsr_mode1_court() {
        let mut n = Noise::new();
        n.write(0x400E, 0x80);
        let p = periode_lfsr(&mut n, 40_000);
        assert!(p == Some(93) || p == Some(31), "periode = {p:?}");
    }

    #[test]
    fn bruit_silence_bit0() {
        let mut n = Noise::new();
        n.write(0x400C, 0x1A); // volume constant 10
        n.lfsr = 1;
        assert_eq!(n.output(), 0);
        n.lfsr = 2;
        assert_eq!(n.output(), 10);
    }

    #[test]
    fn bruit_periode() {
        let mut n = Noise::new();
        n.write(0x400E, 0x02); // 16 cycles
        let depart = n.lfsr;
        n.clock_timer(); // 1er pas immediat (timer a 0)
        let apres1 = n.lfsr;
        assert_ne!(apres1, depart);
        for _ in 0..15 {
            n.clock_timer();
        }
        assert_eq!(n.lfsr, apres1); // pas de nouveau pas avant 16 cycles
        n.clock_timer();
        assert_ne!(n.lfsr, apres1);
    }
}
