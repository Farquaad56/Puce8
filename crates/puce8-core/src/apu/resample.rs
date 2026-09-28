//! Reechantillonnage (E34a2) : moyenne des sorties de chaque cycle CPU sur une periode de sortie.

/// Frequence NTSC du CPU (Hz).
pub const CPU_HZ: f64 = 1_789_773.0;

/// Accumulateur fractionnaire : un echantillon tous les `CPU_HZ / taux` cycles.
#[derive(Debug, Clone, PartialEq)]
pub struct Resampler {
    step: f64,
    frac: f64,
    sum: f64,
    n: u32,
}

impl Resampler {
    /// `rate` : taux de sortie (44 100 Hz par defaut, ou celui du peripherique).
    pub fn new(rate: u32) -> Self {
        Resampler {
            step: CPU_HZ / f64::from(rate.max(1)),
            frac: 0.0,
            sum: 0.0,
            n: 0,
        }
    }

    /// Une sortie du mixeur (1 cycle CPU) ; renvoie un echantillon (la moyenne) quand il est du.
    pub fn push(&mut self, x: f32) -> Option<f32> {
        self.sum += f64::from(x);
        self.n += 1;
        self.frac += 1.0;
        if self.frac < self.step {
            return None;
        }
        self.frac -= self.step;
        let m = self.sum / f64::from(self.n);
        self.sum = 0.0;
        self.n = 0;
        Some(m as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taux_echantillons() {
        let mut r = Resampler::new(44_100);
        let n = (0..1_789_773).filter(|_| r.push(0.0).is_some()).count();
        assert!((44_099..=44_101).contains(&n), "n = {n}");
        let mut r = Resampler::new(48_000);
        let n = (0..1_789_773).filter(|_| r.push(0.0).is_some()).count();
        assert!((47_999..=48_001).contains(&n), "n = {n}");
    }

    #[test]
    fn moyenne() {
        let mut r = Resampler::new(44_100);
        let s: Vec<f32> = (0..10_000).filter_map(|i| r.push((i % 2) as f32)).collect();
        assert!(s.iter().all(|&v| (v - 0.5).abs() < 0.03)); // alternance 0/1 -> environ 0,5
    }
}
