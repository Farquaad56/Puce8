//! Mixeur non lineaire de l'APU (E34a1) : tables pulse et tnd.
// wiki: APU_Mixer

/// Tables du mixeur.
#[derive(Debug, Clone, PartialEq)]
pub struct Mixer {
    pulse: [f32; 31],
    tnd: [f32; 203],
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
    /// pulse[n] = 95.52 / (8128 / n + 100) ; tnd[n] = 163.67 / (24329 / n + 100) ; [0] = 0.
    pub fn new() -> Self {
        let mut pulse = [0.0f32; 31];
        for (n, v) in pulse.iter_mut().enumerate().skip(1) {
            *v = 95.52 / (8128.0 / n as f32 + 100.0);
        }
        let mut tnd = [0.0f32; 203];
        for (n, v) in tnd.iter_mut().enumerate().skip(1) {
            *v = 163.67 / (24329.0 / n as f32 + 100.0);
        }
        Mixer { pulse, tnd }
    }

    /// Sortie (0.0 - 1.0) a partir de [pulse 1, pulse 2, triangle, bruit, DMC].
    pub fn mix(&self, o: [u8; 5]) -> f32 {
        let p = usize::from(o[0]) + usize::from(o[1]);
        let t = 3 * usize::from(o[2]) + 2 * usize::from(o[3]) + usize::from(o[4]);
        self.pulse[p.min(30)] + self.tnd[t.min(202)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixeur_silence() {
        assert_eq!(Mixer::new().mix([0; 5]), 0.0);
    }

    #[test]
    fn mixeur_pulse_max() {
        let v = Mixer::new().mix([15, 15, 0, 0, 0]);
        assert!((v - 0.2575).abs() < 0.001, "v = {v}");
    }

    #[test]
    fn mixeur_tnd_max() {
        let v = Mixer::new().mix([0, 0, 15, 15, 127]);
        assert!((v - 0.7425).abs() < 0.002, "v = {v}");
    }
}
