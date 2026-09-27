//! Manettes standard `$4016`/`$4017` : strobe + registre a decalage (E24a1, logique pure).
//! Branchement sur le bus et `Nes::set_buttons` : E24a2.
// wiki: Standard_controller

/// Boutons : bit 0 = A, 1 = B, 2 = Select, 3 = Start, 4 = Haut, 5 = Bas, 6 = Gauche, 7 = Droite.
pub const BTN_A: u8 = 0x01;
pub const BTN_B: u8 = 0x02;
pub const BTN_SELECT: u8 = 0x04;
pub const BTN_START: u8 = 0x08;
pub const BTN_UP: u8 = 0x10;
pub const BTN_DOWN: u8 = 0x20;
pub const BTN_LEFT: u8 = 0x40;
pub const BTN_RIGHT: u8 = 0x80;

/// Un port de manette ($4016 = manette 1, $4017 = manette 2).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    /// Etat courant des boutons (pose par le frontend).
    pub buttons: u8,
    /// Strobe (bit 0 de $4016) : tant qu'il est haut, le registre est recharge en continu.
    strobe: bool,
    /// Registre a decalage (boutons verrouilles a la descente du strobe).
    shift: u8,
    /// Nombre de bits deja lus depuis le verrouillage (>= 8 : la ligne renvoie 1).
    count: u8,
}

impl Port {
    /// Ecriture du strobe (bit 0). Strobe haut ou front descendant : recharge du registre.
    pub fn write_strobe(&mut self, value: u8) {
        let was = self.strobe;
        self.strobe = value & 1 != 0;
        if self.strobe || was {
            self.shift = self.buttons;
            self.count = 0;
        }
    }

    /// Bit 0 de la prochaine lecture, sans effet de bord.
    fn next_bit(&self) -> u8 {
        if self.strobe {
            self.buttons & 1 // recharge en continu : toujours le bouton A
        } else if self.count >= 8 {
            1 // apres 8 lectures : ligne de donnee haute
        } else {
            self.shift & 1
        }
    }

    /// Lecture : `(open_bus & 0xE0) | bit`, puis decalage (sauf si strobe haut).
    pub fn read(&mut self, open_bus: u8) -> u8 {
        let bit = self.next_bit();
        if !self.strobe && self.count < 8 {
            self.shift >>= 1;
            self.count += 1;
        }
        (open_bus & 0xE0) | bit
    }

    /// Lecture sans effet de bord : meme valeur que `read`, sans decalage.
    pub fn peek(&self, open_bus: u8) -> u8 {
        (open_bus & 0xE0) | self.next_bit()
    }
}

/// Les deux manettes standard.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Controller {
    /// Port 0 = $4016 (manette 1), port 1 = $4017 (manette 2).
    pub ports: [Port; 2],
}

impl Controller {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pose les boutons d'un port (0 ou 1) ; port hors plage ignore.
    pub fn set_buttons(&mut self, port: usize, buttons: u8) {
        if let Some(p) = self.ports.get_mut(port) {
            p.buttons = buttons;
        }
    }

    /// W $4016 : le bit 0 (strobe) va aux DEUX manettes.
    pub fn write(&mut self, value: u8) {
        for p in &mut self.ports {
            p.write_strobe(value);
        }
    }

    /// R $4016 (port 0) / $4017 (port 1).
    pub fn read(&mut self, port: usize, open_bus: u8) -> u8 {
        match self.ports.get_mut(port) {
            Some(p) => p.read(open_bus),
            None => open_bus,
        }
    }

    /// Lecture sans effet de bord.
    pub fn peek(&self, port: usize, open_bus: u8) -> u8 {
        match self.ports.get(port) {
            Some(p) => p.peek(open_bus),
            None => open_bus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strobe 1 puis 0 (comme un jeu), puis `n` lectures du port (bit 0 seulement).
    fn lire(c: &mut Controller, port: usize, n: usize) -> Vec<u8> {
        c.write(1);
        c.write(0);
        (0..n).map(|_| c.read(port, 0) & 1).collect()
    }

    #[test]
    fn ordre_bits() {
        let mut c = Controller::new();
        c.set_buttons(0, BTN_A | BTN_START); // 0b0000_1001
        assert_eq!(lire(&mut c, 0, 8), vec![1, 0, 0, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn apres_8() {
        let mut c = Controller::new();
        c.set_buttons(0, 0);
        assert_eq!(lire(&mut c, 0, 10), vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 1]);
    }

    #[test]
    fn strobe_haut() {
        let mut c = Controller::new();
        c.set_buttons(0, BTN_A);
        c.write(1); // strobe reste haut
        assert_eq!((c.read(0, 0), c.read(0, 0), c.read(0, 0)), (1, 1, 1));
    }

    #[test]
    fn verrouillage_a_la_descente() {
        let mut c = Controller::new();
        c.set_buttons(0, BTN_B);
        c.write(1);
        c.write(0); // B verrouille ici
        c.set_buttons(0, BTN_A); // change apres : n'affecte pas la sequence en cours
        assert_eq!((c.read(0, 0), c.read(0, 0)), (0, 1));
    }

    #[test]
    fn deux_manettes() {
        let mut c = Controller::new();
        c.set_buttons(0, BTN_RIGHT);
        c.set_buttons(1, BTN_A);
        c.write(1);
        c.write(0); // le strobe va aux deux ports
        assert_eq!(c.read(1, 0) & 1, 1); // manette 2 : A
        assert_eq!(c.read(0, 0) & 1, 0); // manette 1 : A non presse
    }

    #[test]
    fn open_bus_et_peek() {
        let mut c = Controller::new();
        c.set_buttons(0, BTN_A);
        c.write(1);
        c.write(0);
        assert_eq!(c.peek(0, 0x5F), 0x41); // (0x5F & 0xE0) | 1, sans decalage
        assert_eq!(c.read(0, 0x5F), 0x41);
        assert_eq!(c.read(0, 0x40), 0x40); // B non presse
    }

    // ---------- E24a2 : manettes branchees sur le bus et le CPU ----------

    use crate::cpu::CpuBus;
    use crate::nes::Nes;

    /// ROM iNES : PRG-ROM 16 Ko de NOP, `code` en $8000, vecteur RESET = $8000 ; reset joue (7 cycles).
    fn machine(code: &[u8]) -> Nes {
        let mut rom = vec![0x4E, 0x45, 0x53, 0x1A, 0x01, 0x00];
        rom.extend_from_slice(&[0; 10]);
        let mut prg = vec![0xEA; 16384];
        prg[..code.len()].copy_from_slice(code);
        prg[0x3FFC..0x3FFE].copy_from_slice(&[0x00, 0x80]);
        rom.extend(prg);
        let mut nes = Nes::from_rom(&rom).unwrap();
        for _ in 0..7 {
            nes.tick();
        }
        nes
    }

    #[test]
    fn bus_ordre_bits() {
        let mut nes = machine(&[]);
        nes.set_buttons(0, BTN_A | BTN_START);
        nes.bus.write(0x4016, 1);
        nes.bus.write(0x4016, 0);
        let bits: Vec<u8> = (0..9).map(|_| nes.bus.read(0x4016) & 1).collect();
        assert_eq!(bits, vec![1, 0, 0, 1, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn bus_manette_2() {
        let mut nes = machine(&[]);
        nes.set_buttons(1, BTN_B);
        nes.bus.write(0x4016, 1);
        nes.bus.write(0x4016, 0);
        assert_eq!(nes.bus.read(0x4017) & 1, 0); // A
        assert_eq!(nes.bus.read(0x4017) & 1, 1); // B
        assert_eq!(nes.peek(0x4017) & 1, 0); // Select, sans decalage
    }

    #[test]
    fn open_bus_bits() {
        // LDA $4016 execute par le CPU : bits 5-7 = $40 & 0xE0 (octet haut de l'operande).
        let mut nes = machine(&[
            0xA9, 0x01, 0x8D, 0x16, 0x40, 0xA9, 0x00, 0x8D, 0x16, 0x40, 0xAD, 0x16, 0x40,
        ]);
        nes.set_buttons(0, BTN_A);
        for _ in 0..5 {
            nes.step_instruction(); // LDA #1 ; STA $4016 ; LDA #0 ; STA $4016 ; LDA $4016
        }
        assert_eq!(nes.cpu.a, 0x41);
    }
}
