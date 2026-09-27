//! Compteurs de longueur de l'APU (E30a1) : table, chargement, decompte (demi-trame), halt.
// wiki: APU_Length_Counter

/// Valeurs chargees ; index = bits 7-3 de $4003/$4007/$400B/$400F.
pub const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

/// Compteur de longueur d'un canal.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LengthCounter {
    pub counter: u8,
    /// Halt : le compteur ne decompte plus ($4000/$4004/$400C bit 5, $4008 bit 7).
    pub halt: bool,
    /// Canal active par $4015.
    pub enabled: bool,
}

impl LengthCounter {
    /// Charge `LENGTH_TABLE[index]`, seulement si le canal est active.
    pub fn load(&mut self, index: u8) {
        if self.enabled {
            self.counter = LENGTH_TABLE[usize::from(index & 0x1F)];
        }
    }

    /// Horloge demi-trame : -1 si > 0 et pas en halt.
    pub fn clock(&mut self) {
        if self.counter > 0 && !self.halt {
            self.counter -= 1;
        }
    }

    /// $4015 : desactiver remet le compteur a 0.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.counter = 0;
        }
    }

    /// Compteur non nul (bit de $4015 en lecture ; canal audible).
    pub fn active(&self) -> bool {
        self.counter > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actif() -> LengthCounter {
        LengthCounter {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn table_longueur() {
        assert_eq!(LENGTH_TABLE[1], 254);
        assert_eq!(LENGTH_TABLE[0x1F], 30);
        let mut l = actif();
        l.load(1);
        assert_eq!(l.counter, 254);
    }

    #[test]
    fn longueur_decompte() {
        let mut l = actif();
        l.load(0); // 10
        l.clock();
        l.clock();
        assert_eq!(l.counter, 8);
        for _ in 0..20 {
            l.clock();
        }
        assert_eq!(l.counter, 0); // s'arrete a 0
        assert!(!l.active());
    }

    #[test]
    fn halt() {
        let mut l = actif();
        l.load(0);
        l.halt = true;
        l.clock();
        assert_eq!(l.counter, 10);
    }

    #[test]
    fn desactivation() {
        let mut l = actif();
        l.load(1);
        l.set_enabled(false);
        assert_eq!(l.counter, 0);
        l.load(1); // ignore : canal desactive
        assert_eq!(l.counter, 0);
    }
}
