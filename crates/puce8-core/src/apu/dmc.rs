//! Canal DMC (E33a1/E33a2) : unite de sortie delta 7 bits et lecteur de memoire.
//! Les octets sont lus par la DMC DMA du bus (E33b1) via `dma_request` / `dma_complete`.
// wiki: APU_DMC

/// Debits NTSC en cycles CPU par bit ($4010 bits 0-3).
pub const DMC_RATES: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

/// Canal DMC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dmc {
    /// I : IRQ en fin d'echantillon.
    pub irq_enabled: bool,
    /// L : boucle.
    pub looping: bool,
    /// Periode du timer en cycles CPU.
    pub rate: u16,
    timer: u16,
    /// Niveau de sortie (0-127).
    pub level: u8,
    /// $4012 : adresse de depart ($C000 + A*64).
    pub sample_addr: u16,
    /// $4013 : longueur (L*16 + 1).
    pub sample_len: u16,
    /// Lecteur : prochaine adresse et octets restants (E33a2).
    pub current_addr: u16,
    pub remaining: u16,
    /// Buffer d'un octet (None = vide).
    pub buffer: Option<u8>,
    shift: u8,
    bits: u8,
    silence: bool,
    /// Drapeau d'IRQ du DMC ($4015 bit 7).
    pub irq: bool,
}

impl Default for Dmc {
    fn default() -> Self {
        Self::new()
    }
}

impl Dmc {
    pub fn new() -> Self {
        Dmc {
            irq_enabled: false,
            looping: false,
            rate: DMC_RATES[0],
            timer: 0,
            level: 0,
            sample_addr: 0xC000,
            sample_len: 1,
            current_addr: 0xC000,
            remaining: 0,
            buffer: None,
            shift: 0,
            bits: 8,
            silence: true,
            irq: false,
        }
    }

    /// Ecriture d'un registre (`addr & 3`) : $4010 `IL-- RRRR`, $4011 niveau, $4012 adresse, $4013 longueur.
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr & 3 {
            0 => {
                self.irq_enabled = v & 0x80 != 0;
                self.looping = v & 0x40 != 0;
                self.rate = DMC_RATES[usize::from(v & 0x0F)];
                if !self.irq_enabled {
                    self.irq = false;
                }
            }
            1 => self.level = v & 0x7F,
            2 => self.sample_addr = 0xC000 | (u16::from(v) << 6),
            _ => self.sample_len = (u16::from(v) << 4) | 1,
        }
    }

    /// Un cycle CPU : fin de periode -> un bit de l'unite de sortie.
    pub fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.rate.saturating_sub(1);
            self.clock_output();
        } else {
            self.timer -= 1;
        }
    }

    /// Un bit de l'unite de sortie : niveau +/- 2 (borne 0-127), decalage, rechargement tous les 8 bits.
    pub fn clock_output(&mut self) {
        if !self.silence {
            if self.shift & 1 == 1 {
                if self.level <= 125 {
                    self.level += 2;
                }
            } else if self.level >= 2 {
                self.level -= 2;
            }
        }
        self.shift >>= 1;
        self.bits -= 1;
        if self.bits == 0 {
            self.bits = 8;
            match self.buffer.take() {
                None => self.silence = true,
                Some(b) => {
                    self.silence = false;
                    self.shift = b;
                }
            }
        }
    }

    /// Sortie 0-127.
    pub fn output(&self) -> u8 {
        self.level
    }

    // ---------- E33a2 : lecteur de memoire ----------

    fn restart(&mut self) {
        self.current_addr = self.sample_addr;
        self.remaining = self.sample_len;
    }

    /// $4015 bit 4 : 0 -> plus rien a lire ; 1 avec 0 octet restant -> redemarrage de l'echantillon.
    pub fn set_enabled(&mut self, on: bool) {
        if !on {
            self.remaining = 0;
        } else if self.remaining == 0 {
            self.restart();
        }
    }

    /// Adresse a lire : buffer vide et octets restants (sinon None).
    pub fn dma_request(&self) -> Option<u16> {
        if self.buffer.is_none() && self.remaining > 0 {
            Some(self.current_addr)
        } else {
            None
        }
    }

    /// Octet lu par la DMC DMA : buffer plein, adresse + 1 ($FFFF -> $8000), fin -> boucle ou IRQ.
    pub fn dma_complete(&mut self, v: u8) {
        self.buffer = Some(v);
        self.current_addr = if self.current_addr == 0xFFFF {
            0x8000
        } else {
            self.current_addr + 1
        };
        self.remaining = self.remaining.saturating_sub(1);
        if self.remaining == 0 {
            if self.looping {
                self.restart();
            } else if self.irq_enabled {
                self.irq = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_longueur() {
        let mut d = Dmc::new();
        d.write(0x4012, 0x01);
        d.write(0x4013, 0x01);
        assert_eq!(d.sample_addr, 0xC040);
        assert_eq!(d.sample_len, 17);
        d.write(0x4010, 0x0F);
        assert_eq!(d.rate, 54);
    }

    #[test]
    fn niveau_monte() {
        let mut d = Dmc::new();
        d.level = 64;
        d.buffer = Some(0xFF);
        for _ in 0..8 {
            d.clock_output(); // silence ; au 8e bit, le buffer passe dans le registre
        }
        assert_eq!(d.level, 64);
        for _ in 0..8 {
            d.clock_output();
        }
        assert_eq!(d.level, 80);
    }

    #[test]
    fn niveau_borne() {
        let mut d = Dmc::new();
        d.level = 126;
        d.buffer = Some(0xFF);
        for _ in 0..16 {
            d.clock_output();
        }
        assert_eq!(d.level, 126); // 126 + 2 > 127 : inchange
        d.level = 1;
        d.buffer = Some(0x00);
        for _ in 0..16 {
            d.clock_output();
        }
        assert_eq!(d.level, 1); // 1 - 2 < 0 : inchange
    }

    // ---------- E33a2 ----------

    #[test]
    fn lecteur_demarre() {
        let mut d = Dmc::new();
        d.write(0x4012, 0x01);
        d.write(0x4013, 0x01);
        assert_eq!(d.dma_request(), None); // pas active
        d.set_enabled(true);
        assert_eq!(d.dma_request(), Some(0xC040));
        d.dma_complete(0x55);
        assert_eq!(d.buffer, Some(0x55));
        assert_eq!(d.remaining, 16);
        assert_eq!(d.dma_request(), None); // buffer plein
        d.set_enabled(false);
        assert_eq!(d.remaining, 0);
    }

    #[test]
    fn irq_fin() {
        let mut d = Dmc::new();
        d.write(0x4010, 0x80); // IRQ activee
        d.write(0x4013, 0x00); // 1 octet
        d.set_enabled(true);
        d.dma_complete(0);
        assert!(d.irq);
        d.write(0x4010, 0x00); // I = 0 : efface le drapeau
        assert!(!d.irq);
    }

    #[test]
    fn boucle() {
        let mut d = Dmc::new();
        d.write(0x4010, 0xC0); // IRQ + boucle
        d.write(0x4013, 0x00);
        d.set_enabled(true);
        d.dma_complete(0);
        assert_eq!(d.remaining, 1); // redemarre
        assert_eq!(d.current_addr, 0xC000);
        assert!(!d.irq); // pas d'IRQ en boucle
    }

    #[test]
    fn wrap_adresse() {
        let mut d = Dmc::new();
        d.current_addr = 0xFFFF;
        d.remaining = 2;
        d.dma_complete(0);
        assert_eq!(d.current_addr, 0x8000);
    }
}
