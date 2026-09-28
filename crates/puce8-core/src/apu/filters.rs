//! Filtres de sortie (E34a3) : passe-haut 90 Hz, passe-haut 440 Hz, passe-bas 14 kHz (1er ordre).
// wiki: APU_Mixer

use std::f32::consts::PI;

/// Passe-haut du 1er ordre : y = a * (y_prec + x - x_prec), a = RC / (RC + dt).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighPass {
    a: f32,
    prev_x: f32,
    prev_y: f32,
}

impl HighPass {
    pub fn new(fc: f32, rate: f32) -> Self {
        let rc = 1.0 / (2.0 * PI * fc);
        let dt = 1.0 / rate;
        HighPass {
            a: rc / (rc + dt),
            prev_x: 0.0,
            prev_y: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.a * (self.prev_y + x - self.prev_x);
        self.prev_x = x;
        self.prev_y = y;
        y
    }
}

/// Passe-bas du 1er ordre : y += b * (x - y), b = dt / (RC + dt).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LowPass {
    b: f32,
    y: f32,
}

impl LowPass {
    pub fn new(fc: f32, rate: f32) -> Self {
        let rc = 1.0 / (2.0 * PI * fc);
        let dt = 1.0 / rate;
        LowPass {
            b: dt / (rc + dt),
            y: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        self.y += self.b * (x - self.y);
        self.y
    }
}

/// Chaine de la NES : passe-haut 90 Hz -> passe-haut 440 Hz -> passe-bas 14 kHz, ecretage [-1, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Filters {
    hp90: HighPass,
    hp440: HighPass,
    lp14k: LowPass,
}

impl Filters {
    pub fn new(rate: u32) -> Self {
        let r = rate.max(1) as f32;
        Filters {
            hp90: HighPass::new(90.0, r),
            hp440: HighPass::new(440.0, r),
            lp14k: LowPass::new(14_000.0, r),
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.lp14k.process(self.hp440.process(self.hp90.process(x)));
        y.clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moyenne_nulle() {
        let mut f = Filters::new(44_100);
        let mut y = 1.0;
        for _ in 0..44_100 {
            y = f.process(0.5); // signal continu : supprime par les passe-haut
        }
        assert!(y.abs() < 0.01, "y = {y}");
    }

    #[test]
    fn ecretage() {
        let mut f = Filters::new(44_100);
        assert!(f.process(5.0) <= 1.0);
        assert!(f.process(-10.0) >= -1.0);
    }

    #[test]
    fn passe_bas_attenue() {
        let mut lp = LowPass::new(14_000.0, 44_100.0);
        let mut amp = 0.0f32;
        for i in 0..1000u32 {
            let x = if i.is_multiple_of(2) { 1.0 } else { -1.0 }; // 22 050 Hz
            let y = lp.process(x);
            if i > 900 {
                amp = amp.max(y.abs());
            }
        }
        assert!(amp < 0.6, "amp = {amp}");
    }
}
