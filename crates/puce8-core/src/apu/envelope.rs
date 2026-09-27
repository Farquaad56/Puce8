//! Enveloppe de volume (E31a1) : pulses et bruit, horloge "quart".
// wiki: APU_Envelope

/// Enveloppe (`--LC VVVV`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Envelope {
    /// Redemarrage demande (ecriture du 4e registre du canal).
    pub start: bool,
    /// L : boucle (c'est aussi le halt du compteur de longueur).
    pub looping: bool,
    /// C : volume constant.
    pub constant: bool,
    /// V : volume constant, ou periode du diviseur.
    pub volume: u8,
    decay: u8,
    divider: u8,
}

impl Envelope {
    /// Ecriture `--LC VVVV` ($4000/$4004/$400C).
    pub fn write(&mut self, v: u8) {
        self.looping = v & 0x20 != 0;
        self.constant = v & 0x10 != 0;
        self.volume = v & 0x0F;
    }

    /// Horloge "quart".
    pub fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0 {
            self.divider = self.volume;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looping {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    /// Volume 0-15.
    pub fn output(&self) -> u8 {
        if self.constant {
            self.volume
        } else {
            self.decay
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enveloppe_decroit() {
        let mut e = Envelope::default();
        e.write(0x00); // V = 0 : decroit a chaque horloge
        e.start = true;
        let v: Vec<u8> = (0..4)
            .map(|_| {
                e.clock();
                e.output()
            })
            .collect();
        assert_eq!(v, vec![15, 14, 13, 12]);
    }

    #[test]
    fn enveloppe_diviseur() {
        let mut e = Envelope::default();
        e.write(0x02); // V = 2 : decroit toutes les 3 horloges
        e.start = true;
        e.clock(); // 15
        e.clock();
        e.clock();
        assert_eq!(e.output(), 15);
        e.clock();
        assert_eq!(e.output(), 14);
    }

    #[test]
    fn enveloppe_boucle() {
        let mut e = Envelope::default();
        e.write(0x20); // boucle, V = 0
        e.start = true;
        for _ in 0..16 {
            e.clock(); // 15, 14, ..., 0
        }
        assert_eq!(e.output(), 0);
        e.clock();
        assert_eq!(e.output(), 15); // repart a 15
        let mut f = Envelope::default();
        f.write(0x00); // sans boucle : reste a 0
        f.start = true;
        for _ in 0..40 {
            f.clock();
        }
        assert_eq!(f.output(), 0);
    }

    #[test]
    fn volume_constant() {
        let mut e = Envelope::default();
        e.write(0x1A);
        e.start = true;
        e.clock();
        assert_eq!(e.output(), 10);
    }
}
