//! MMC1 (mapper 1) : registre serie 5 bits, Control, banques PRG/CHR, PRG-RAM, SUROM.
//! E26a1 : registre serie seul ; le mapper `Mmc1` arrive en E26a2.
// wiki: MMC1 ; wiki: INES_Mapper_001

// ---------------------------------------------------------------- E26a1 : registre serie

/// Resultat d'une ecriture dans le registre serie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialEvent {
    /// Bit accumule, ou ecriture ignoree (cycle consecutif).
    Rien,
    /// Bit 7 = 1 : registre vide ; l'appelant fait `control |= 0x0C`.
    Reset,
    /// 5e ecriture : (registre 0..=3 = bits 13-14 de l'adresse, valeur 5 bits).
    Ecrit(u8, u8),
}

/// Registre a decalage du MMC1 (bit de poids faible en premier).
#[derive(Debug, Default, Clone, Copy)]
pub struct Serial {
    shift: u8,
    count: u8,
    /// Compteur de cycles CPU (incremente par `tick`, appele depuis `Mapper::cpu_cycle`).
    cycle: u64,
    /// Cycle de la derniere ecriture (None = aucune).
    last_write: Option<u64>,
}

impl Serial {
    /// Un cycle CPU (M2).
    pub fn tick(&mut self) {
        self.cycle += 1;
    }

    /// Ecriture CPU en $8000-$FFFF.
    pub fn write(&mut self, addr: u16, value: u8) -> SerialEvent {
        // Ecritures sur des cycles consecutifs (instructions RMW) : seule la 1re compte.
        let ignoree = matches!(self.last_write, Some(c) if self.cycle == c + 1);
        self.last_write = Some(self.cycle);
        if ignoree {
            return SerialEvent::Rien;
        }
        if value & 0x80 != 0 {
            self.shift = 0;
            self.count = 0;
            return SerialEvent::Reset;
        }
        self.shift |= (value & 1) << self.count;
        self.count += 1;
        if self.count < 5 {
            return SerialEvent::Rien;
        }
        let v = self.shift;
        self.shift = 0;
        self.count = 0;
        SerialEvent::Ecrit(((addr >> 13) & 3) as u8, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 5 ecritures de `v5` (poids faible d'abord), 2 cycles avant chacune ; renvoie le dernier evenement.
    fn serie(s: &mut Serial, addr: u16, v5: u8) -> SerialEvent {
        let mut ev = SerialEvent::Rien;
        for i in 0..5 {
            s.tick();
            s.tick();
            ev = s.write(addr, (v5 >> i) & 1);
        }
        ev
    }

    #[test]
    fn serie_cinq_bits() {
        let mut s = Serial::default();
        assert_eq!(
            serie(&mut s, 0xE000, 0b10110),
            SerialEvent::Ecrit(3, 0b10110)
        );
        // Registre vide apres la 5e ecriture : la valeur suivante repart de zero.
        assert_eq!(
            serie(&mut s, 0xE000, 0b00001),
            SerialEvent::Ecrit(3, 0b00001)
        );
    }

    #[test]
    fn serie_registre_par_adresse() {
        let mut s = Serial::default();
        assert_eq!(serie(&mut s, 0x8000, 1), SerialEvent::Ecrit(0, 1));
        assert_eq!(serie(&mut s, 0xBFFF, 2), SerialEvent::Ecrit(1, 2));
        assert_eq!(serie(&mut s, 0xC123, 3), SerialEvent::Ecrit(2, 3));
        assert_eq!(serie(&mut s, 0xFFFF, 4), SerialEvent::Ecrit(3, 4));
    }

    #[test]
    fn serie_reset() {
        let mut s = Serial::default();
        s.tick();
        s.tick();
        assert_eq!(s.write(0x8000, 1), SerialEvent::Rien); // 1 bit en attente...
        s.tick();
        s.tick();
        assert_eq!(s.write(0x8000, 0x80), SerialEvent::Reset); // ... efface
        assert_eq!(
            serie(&mut s, 0xE000, 0b00010),
            SerialEvent::Ecrit(3, 0b00010)
        );
    }

    #[test]
    fn serie_consecutives() {
        let mut s = Serial::default();
        s.tick();
        s.tick();
        assert_eq!(s.write(0xE000, 1), SerialEvent::Rien);
        s.tick(); // cycle suivant : ecriture ignoree (meme un reset)
        assert_eq!(s.write(0xE000, 0x80), SerialEvent::Rien);
        let mut ev = SerialEvent::Rien;
        for _ in 0..4 {
            s.tick();
            s.tick();
            ev = s.write(0xE000, 0);
        }
        assert_eq!(ev, SerialEvent::Ecrit(3, 1));
    }
}
