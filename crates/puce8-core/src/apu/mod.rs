//! APU 2A03 : compteurs de longueur, frame counter, $4015 (E30) ; pulses (E31) ; triangle et
//! bruit (E32). DMC : E33.
// wiki: APU

pub mod length;

use length::LengthCounter;

/// APU.
pub struct Apu {
    /// Compteurs de longueur : 0 = pulse 1, 1 = pulse 2, 2 = triangle, 3 = bruit.
    pub lengths: [LengthCounter; 4],
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

/// Canal (0-3) d'un registre $4000-$400F.
fn canal(addr: u16) -> usize {
    usize::from((addr >> 2) & 3)
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            lengths: [LengthCounter::default(); 4],
        }
    }

    /// Un cycle CPU (frame counter : E30b2).
    pub fn tick(&mut self) {}

    /// Ecriture CPU en $4000-$4013, $4015, $4017.
    pub fn write_register(&mut self, addr: u16, v: u8) {
        self.write_length(addr, v);
        if addr == 0x4015 {
            self.write_status(v);
        }
    }

    /// Bits halt et chargement des compteurs de longueur.
    fn write_length(&mut self, addr: u16, v: u8) {
        match addr {
            0x4000 | 0x4004 | 0x400C => self.lengths[canal(addr)].halt = v & 0x20 != 0,
            0x4008 => self.lengths[2].halt = v & 0x80 != 0,
            0x4003 | 0x4007 | 0x400B | 0x400F => self.lengths[canal(addr)].load(v >> 3),
            _ => {}
        }
    }

    /// W $4015 `---D NT21` : active/desactive les canaux (DMC : E33).
    fn write_status(&mut self, v: u8) {
        for (i, l) in self.lengths.iter_mut().enumerate() {
            l.set_enabled(v & (1 << i) != 0);
        }
    }

    /// Bits de $4015 : N/T/2/1 = longueur > 0 (IRQ de trame, bit 6 : E30c1).
    fn status_bits(&self) -> u8 {
        self.lengths
            .iter()
            .enumerate()
            .fold(0u8, |s, (i, l)| s | (u8::from(l.active()) << i))
    }

    /// R $4015 (effacement de l'IRQ de trame : E30c1).
    pub fn read_status(&mut self) -> u8 {
        self.status_bits()
    }

    /// Lecture de $4015 sans effet de bord.
    pub fn peek_status(&self) -> u8 {
        self.status_bits()
    }

    /// Ligne IRQ de l'APU (IRQ de trame : E30b2).
    pub fn irq_line(&self) -> bool {
        false
    }

    /// Reset a chaud : $4015 = 0 (le reste en E35).
    pub fn reset(&mut self) {
        self.write_status(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registres_longueur() {
        let mut apu = Apu::new();
        apu.write_register(0x4003, 0x08); // ignore : canal desactive
        assert_eq!(apu.peek_status() & 0x0F, 0);
        apu.write_register(0x4015, 0x0F);
        apu.write_register(0x4003, 0x08); // pulse 1 : index 1 -> 254
        apu.write_register(0x400B, 0x08); // triangle
        assert_eq!(apu.lengths[0].counter, 254);
        assert_eq!(apu.peek_status() & 0x0F, 0x05);
        apu.write_register(0x4015, 0x01); // triangle desactive
        assert_eq!(apu.peek_status() & 0x0F, 0x01);
    }

    #[test]
    fn halt_bits() {
        let mut apu = Apu::new();
        apu.write_register(0x4004, 0x20);
        apu.write_register(0x4008, 0x80);
        apu.write_register(0x400C, 0x20);
        assert!(apu.lengths[1].halt && apu.lengths[2].halt && apu.lengths[3].halt);
        assert!(!apu.lengths[0].halt);
        apu.write_register(0x4004, 0x00);
        assert!(!apu.lengths[1].halt);
    }
}
