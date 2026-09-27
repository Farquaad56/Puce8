//! Canal triangle (E32a1) : timer 11 bits (chaque cycle CPU), sequence de 32 pas, compteur lineaire.
//! Le compteur de longueur est gere par `Apu` (passe a `clock_timer`).
// wiki: APU_Triangle

/// Sequence de 32 pas : 15, 14, ..., 0, 0, 1, ..., 15.
pub const TRI_SEQ: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

/// Canal triangle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Triangle {
    /// C : controle (garde le rechargement arme ; c'est aussi le halt de la longueur).
    pub control: bool,
    /// R : valeur de rechargement du compteur lineaire.
    pub linear_reload_value: u8,
    /// Compteur lineaire.
    pub linear: u8,
    /// Rechargement du compteur lineaire arme (ecriture de $400B).
    pub linear_reload: bool,
    /// Periode du timer (11 bits).
    pub period: u16,
    timer: u16,
    /// Pas courant (0-31).
    pub step: u8,
}

impl Triangle {
    /// Ecriture d'un registre (`addr & 3`) : $4008 `CRRR RRRR`, $400A timer bas, $400B `LLLL LTTT`.
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr & 3 {
            0 => {
                self.control = v & 0x80 != 0;
                self.linear_reload_value = v & 0x7F;
            }
            2 => self.period = (self.period & 0x0700) | u16::from(v),
            3 => {
                self.period = (self.period & 0x00FF) | (u16::from(v & 7) << 8);
                self.linear_reload = true;
            }
            _ => {} // $4009 : inutilise
        }
    }

    /// Un cycle CPU. `length_active` : compteur de longueur > 0.
    pub fn clock_timer(&mut self, length_active: bool) {
        if self.timer == 0 {
            self.timer = self.period;
            // SIMPLIFICATION: periode < 2 (ultrasons) -> sequenceur fige.
            if self.linear > 0 && length_active && self.period >= 2 {
                self.step = (self.step + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }

    /// Horloge "quart" : compteur lineaire.
    pub fn clock_linear(&mut self) {
        if self.linear_reload {
            self.linear = self.linear_reload_value;
        } else if self.linear > 0 {
            self.linear -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    /// Sortie 0-15 : valeur du pas courant (figee quand le sequenceur s'arrete).
    pub fn output(&self) -> u8 {
        TRI_SEQ[usize::from(self.step & 31)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPU_HZ: u32 = 1_789_773;

    /// Triangle de periode `t`, compteur lineaire charge (C = 1, R = 127).
    fn triangle(t: u16) -> Triangle {
        let mut tr = Triangle::default();
        tr.write(0x4008, 0xFF);
        tr.write(0x400A, (t & 0xFF) as u8);
        tr.write(0x400B, (t >> 8) as u8);
        tr.clock_linear();
        tr
    }

    #[test]
    fn triangle_frequence() {
        let mut tr = triangle(126); // 1 789 773 / (32 x 127) = 440,4 Hz
        let mut tours = 0u32;
        for _ in 0..CPU_HZ {
            let avant = tr.step;
            tr.clock_timer(true);
            if avant == 31 && tr.step == 0 {
                tours += 1;
            }
        }
        let f = f64::from(tours);
        assert!((f - 440.4).abs() < 4.4, "f = {f}");
    }

    #[test]
    fn triangle_sequence() {
        assert_eq!(TRI_SEQ[0], 15);
        assert_eq!(TRI_SEQ[15], 0);
        assert_eq!(TRI_SEQ[16], 0);
        assert_eq!(TRI_SEQ[31], 15);
    }

    #[test]
    fn triangle_gel() {
        let mut tr = triangle(126);
        for _ in 0..1000 {
            tr.clock_timer(true);
        }
        let pas = tr.step;
        for _ in 0..1000 {
            tr.clock_timer(false); // longueur a 0 : fige
        }
        assert_eq!(tr.step, pas);
        tr.linear = 0;
        for _ in 0..1000 {
            tr.clock_timer(true); // lineaire a 0 : fige
        }
        assert_eq!(tr.step, pas);
        assert_eq!(tr.output(), TRI_SEQ[usize::from(pas)]); // sortie figee, pas 0
    }

    #[test]
    fn lineaire_reload() {
        let mut tr = Triangle::default();
        tr.write(0x4008, 0x05); // C = 0, R = 5
        tr.write(0x400B, 0x00); // arme le rechargement
        tr.clock_linear();
        assert_eq!(tr.linear, 5);
        assert!(!tr.linear_reload); // C = 0 : desarme
        tr.clock_linear();
        assert_eq!(tr.linear, 4);
        tr.write(0x4008, 0x85); // C = 1
        tr.write(0x400B, 0x00);
        tr.clock_linear();
        tr.clock_linear();
        assert_eq!(tr.linear, 5); // reste arme : recharge a chaque fois
    }
}
