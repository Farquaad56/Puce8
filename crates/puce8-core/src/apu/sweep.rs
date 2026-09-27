//! Unite de sweep des pulses (E31a2), horloge "demi".
// wiki: APU_Sweep

/// Sweep (`EPPP NSSS`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sweep {
    /// E : active.
    pub enabled: bool,
    /// P : periode du diviseur.
    pub divider_period: u8,
    /// N : negation.
    pub negate: bool,
    /// S : decalage.
    pub shift: u8,
    reload: bool,
    divider: u8,
}

impl Sweep {
    /// Ecriture `EPPP NSSS` ($4001/$4005) : arme le rechargement du diviseur.
    pub fn write(&mut self, v: u8) {
        self.enabled = v & 0x80 != 0;
        self.divider_period = (v >> 4) & 7;
        self.negate = v & 0x08 != 0;
        self.shift = v & 7;
        self.reload = true;
    }

    /// Periode cible. Negation : pulse 1 = complement a un (-1 de plus), pulse 2 = complement a deux.
    pub fn target(&self, period: u16, pulse1: bool) -> u16 {
        let change = period >> self.shift;
        if self.negate {
            period
                .saturating_sub(change)
                .saturating_sub(u16::from(pulse1))
        } else {
            period.wrapping_add(change)
        }
    }

    /// Canal muet : periode < 8 ou cible > $7FF (calcule meme si le sweep est desactive).
    pub fn muted(&self, period: u16, pulse1: bool) -> bool {
        period < 8 || self.target(period, pulse1) > 0x7FF
    }

    /// Horloge "demi" : peut modifier la periode du canal.
    pub fn clock(&mut self, period: &mut u16, pulse1: bool) {
        let cible = self.target(*period, pulse1);
        if self.divider == 0 && self.enabled && self.shift != 0 && !self.muted(*period, pulse1) {
            *period = cible;
        }
        if self.divider == 0 || self.reload {
            self.divider = self.divider_period;
            self.reload = false;
        } else {
            self.divider -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_pulse1_vs_2() {
        let mut s = Sweep::default();
        s.write(0x89); // E = 1, P = 0, N = 1, S = 1
        assert_eq!(s.target(0x100, true), 0x7F);
        assert_eq!(s.target(0x100, false), 0x80);
    }

    #[test]
    fn muet_cible() {
        let mut s = Sweep::default();
        s.write(0x01); // desactive, S = 1 : la cible compte quand meme
        assert!(s.muted(0x700, false)); // 0x700 + 0x380 > 0x7FF
        assert!(!s.muted(0x100, false));
        assert!(s.muted(7, false)); // periode < 8
    }

    #[test]
    fn sweep_applique() {
        let mut s = Sweep::default();
        s.write(0x81); // E = 1, P = 0, S = 1
        let mut p = 0x100;
        s.clock(&mut p, false);
        assert_eq!(p, 0x180);
        s.write(0x01); // desactive
        s.clock(&mut p, false);
        assert_eq!(p, 0x180);
    }
}
