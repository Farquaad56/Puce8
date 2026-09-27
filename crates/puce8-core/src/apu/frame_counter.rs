//! Frame counter de l'APU (E30b1), NTSC, au cycle CPU pres : horloges "quart" et "demi", IRQ de trame.
// wiki: APU_Frame_Counter

/// Horloges emises par un cycle du frame counter.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameEvent {
    /// Enveloppes et compteur lineaire.
    pub quarter: bool,
    /// Compteurs de longueur et sweeps.
    pub half: bool,
}

/// Sequenceur du frame counter ($4017 `MI-- ----`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameCounter {
    /// Cycles CPU depuis la derniere remise a zero du sequenceur.
    pub cycle: u32,
    /// M : mode 5 pas.
    pub five_step: bool,
    /// I : IRQ de trame inhibee.
    pub inhibit: bool,
    /// Drapeau d'IRQ de trame ($4015 bit 6).
    pub irq: bool,
    /// Cycles restants avant la remise a zero demandee par $4017 (0 = aucune).
    reset_delay: u8,
}

impl FrameCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// W $4017. `cycle_pair` : parite du cycle CPU de l'ecriture (remise a zero 3 cycles
    /// plus tard si pair, 4 sinon ; reglage fin en E35).
    pub fn write(&mut self, v: u8, cycle_pair: bool) {
        self.five_step = v & 0x80 != 0;
        self.inhibit = v & 0x40 != 0;
        if self.inhibit {
            self.irq = false;
        }
        self.reset_delay = if cycle_pair { 3 } else { 4 };
    }

    fn set_irq(&mut self) {
        if !self.inhibit {
            self.irq = true;
        }
    }

    /// Un cycle CPU.
    pub fn tick(&mut self) -> FrameEvent {
        let mut ev = FrameEvent::default();
        if self.reset_delay > 0 {
            self.reset_delay -= 1;
            if self.reset_delay == 0 {
                self.cycle = 0;
                if self.five_step {
                    ev.quarter = true; // M = 1 : quart + demi au moment de la remise a zero
                    ev.half = true;
                }
                return ev;
            }
        }
        self.cycle += 1;
        match (self.five_step, self.cycle) {
            (_, 7457) | (_, 22371) => ev.quarter = true,
            (_, 14913) => {
                ev.quarter = true;
                ev.half = true;
            }
            (false, 29828) => self.set_irq(),
            (false, 29829) => {
                ev.quarter = true;
                ev.half = true;
                self.set_irq();
            }
            (false, 29830) => {
                self.set_irq();
                self.cycle = 0;
            }
            (true, 37281) => {
                ev.quarter = true;
                ev.half = true;
            }
            (true, 37282) => self.cycle = 0,
            _ => {}
        }
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Joue `n` cycles ; renvoie (nombre de quarts, nombre de demis).
    fn jouer(fc: &mut FrameCounter, n: u32) -> (u32, u32) {
        let (mut q, mut h) = (0, 0);
        for _ in 0..n {
            let ev = fc.tick();
            q += u32::from(ev.quarter);
            h += u32::from(ev.half);
        }
        (q, h)
    }

    #[test]
    fn mode0_evenements() {
        let mut fc = FrameCounter::new();
        assert_eq!(jouer(&mut fc, 7456), (0, 0));
        assert_eq!(jouer(&mut fc, 1), (1, 0)); // 7457 : quart
        assert_eq!(jouer(&mut fc, 29830 - 7457), (3, 2)); // 14913, 22371, 29829
        assert_eq!(fc.cycle, 0); // periode de 29 830 cycles
    }

    #[test]
    fn mode0_irq() {
        let mut fc = FrameCounter::new();
        jouer(&mut fc, 29827);
        assert!(!fc.irq);
        jouer(&mut fc, 1); // 29 828
        assert!(fc.irq);
    }

    #[test]
    fn inhibit() {
        let mut fc = FrameCounter::new();
        jouer(&mut fc, 29830); // IRQ posee
        assert!(fc.irq);
        fc.write(0x40, true); // I = 1 : efface tout de suite
        assert!(!fc.irq);
        jouer(&mut fc, 70_000);
        assert!(!fc.irq);
    }

    #[test]
    fn mode1_pas_d_irq() {
        let mut fc = FrameCounter::new();
        fc.write(0x80, true); // mode 5 pas, cycle pair : remise a zero 3 cycles plus tard
        assert_eq!(jouer(&mut fc, 2), (0, 0));
        assert_eq!(jouer(&mut fc, 1), (1, 1)); // quart + demi immediats
        assert_eq!(jouer(&mut fc, 37282), (4, 2)); // 7457, 14913, 22371, 37281
        assert!(!fc.irq);
    }

    #[test]
    fn delai_impair() {
        let mut fc = FrameCounter::new();
        fc.write(0x80, false); // cycle impair : 4 cycles
        assert_eq!(jouer(&mut fc, 3), (0, 0));
        assert_eq!(jouer(&mut fc, 1), (1, 1));
    }
}
