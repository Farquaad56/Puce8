//! PPU minimale : geometrie 341 x 262, VBlank en (241, 1), registres $2000/$2002 (E13a).
// wiki: PPU_scrolling ; wiki: PPU_masks_and_control

pub mod registers;

use crate::mapper::Mapper;
use registers::Registers;

/// PPU : 341 points (0-340) x 262 lignes (0-261) par image.
pub struct Ppu {
    /// Ligne courante (0-261).
    pub line: u16,
    /// Point courant dans la ligne (0-340).
    pub point: u16,
    /// Image en cours ; 0 = pair. Le drapeau pair/impair bascule a chaque image.
    pub frame: u64,
    /// VBlank : 1 de (241, 1) a (261, 1).
    pub vblank: bool,
    /// true une seule fois par image, au moment ou VBlank passe a 1.
    pub frame_complete: bool,
    /// Registres $2000-$2007.
    pub regs: Registers,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    /// Mise sous tension : (0, 0), image 0 (pair).
    pub fn new() -> Self {
        Ppu {
            line: 0,
            point: 0,
            frame: 0,
            vblank: false,
            frame_complete: false,
            regs: Registers::new(),
        }
    }

    /// Avance d'un point. Apres la ligne 261 : retour a la ligne 0 et `frame += 1`.
    pub fn tick(&mut self, _mapper: &mut dyn Mapper) {
        let (line, point) = (self.line, self.point);
        // Image impaire avec rendu actif : on saute de (261, 339) a (0, 0).
        let fin_image = line == 261 && (point == 340 || self.fin_image_impaire());
        if fin_image {
            self.line = 0;
            self.point = 0;
            self.frame += 1;
        } else if point == 340 {
            self.line += 1;
            self.point = 0;
        } else {
            self.point += 1;
        }
        match (self.line, self.point) {
            (241, 1) => {
                self.vblank = true; // VBlank = 1 en (241, 1)
                self.frame_complete = true;
            }
            (261, 1) => self.vblank = false, // VBlank = 0 en (261, 1)
            _ => {}
        }
    }

    /// Image impaire avec rendu actif (`mask & 0x18 != 0`).
    fn fin_image_impaire(&self) -> bool {
        self.point == 339 && self.frame % 2 == 1 && self.regs.mask & 0x18 != 0
    }

    /// Lecture CPU d'un registre PPU (reg = addr & 7). $2002 efface VBlank et `w`.
    pub fn cpu_read_register(&mut self, reg: u16, _mapper: &mut dyn Mapper) -> u8 {
        let r = usize::from(reg & 0x07);
        let value = self.regs.read(r as u8, self.vblank);
        if r == 2 {
            self.vblank = false; // $2002 efface VBlank
        }
        value
    }

    /// Lecture sans effet : meme valeur que `cpu_read_register`, sans effacer.
    pub fn cpu_peek_register(&self, reg: u16) -> u8 {
        self.regs.peek(usize::from(reg & 0x07) as u8, self.vblank)
    }

    /// Ecriture CPU d'un registre PPU (reg = addr & 7).
    pub fn cpu_write_register(&mut self, reg: u16, v: u8, _mapper: &mut dyn Mapper) {
        self.regs.write(usize::from(reg & 0x07) as u8, v);
    }

    /// Ligne NMI : VBlank active et bit 7 de $2000 pose.
    pub fn nmi_line(&self) -> bool {
        self.vblank && self.regs.ctrl & 0x80 != 0
    }

    /// true une seule fois par image, au moment ou VBlank passe a 1.
    pub fn take_frame_complete(&mut self) -> bool {
        let done = self.frame_complete;
        self.frame_complete = false;
        done
    }

    /// Position courante : (ligne, point).
    pub fn position(&self) -> (u16, u16) {
        (self.line, self.point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;

    /// Tick jusqu'a la position visee (toujours atteignable en moins d'une image).
    fn tick_to(ppu: &mut Ppu, bus: &mut Bus, target: (u16, u16)) {
        while ppu.position() != target {
            ppu.tick(&mut (*bus.mapper));
        }
    }

    #[test]
    fn ppu_geometrie() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new();
        assert_eq!(ppu.position(), (0, 0)); // mise sous tension
        for _ in 0..341 * 262 {
            ppu.tick(&mut (*bus.mapper));
        }
        assert_eq!(ppu.position(), (0, 0));
        assert_eq!(ppu.frame, 1);
    }

    #[test]
    fn vblank_set() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new();
        tick_to(&mut ppu, &mut bus, (241, 0));
        assert!(!ppu.vblank); // encore 0 en (241, 0)
        ppu.tick(&mut (*bus.mapper)); // -> (241, 1)
        assert!(ppu.vblank);
        assert!(ppu.take_frame_complete()); // vrai une seule fois
        assert!(!ppu.take_frame_complete());
    }

    #[test]
    fn vblank_clear() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new();
        tick_to(&mut ppu, &mut bus, (261, 1));
        assert!(!ppu.vblank); // VBlank = 0 en (261, 1)
    }

    #[test]
    fn lecture_2002_efface() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new();
        tick_to(&mut ppu, &mut bus, (241, 1)); // VBlank = 1
        assert_eq!(ppu.cpu_read_register(0x2002, &mut (*bus.mapper)), 0x80);
        assert_eq!(ppu.cpu_read_register(0x2002, &mut (*bus.mapper)), 0x00); // VBlank efface
    }

    #[test]
    fn nmi_ligne() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new();
        tick_to(&mut ppu, &mut bus, (241, 1)); // VBlank = 1
        assert!(!ppu.nmi_line()); // $2000 = 0 -> NMI inactive
        ppu.cpu_write_register(0x2000, 0x80, &mut (*bus.mapper));
        assert!(ppu.nmi_line()); // VBlank + bit 7 de $2000 pose
    }

    #[test]
    fn image_impaire() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new();
        ppu.cpu_write_register(0x2001, 0x18, &mut (*bus.mapper)); // rendu actif
                                                                  // Image pair : 341 x 262 points ; image impaire : un point de moins.
        for _ in 0..(341 * 262 + 341 * 262 - 1) {
            ppu.tick(&mut (*bus.mapper));
        }
        assert_eq!(ppu.position(), (0, 0));
        assert_eq!(ppu.frame, 2);
    }

    #[test]
    fn image_impaire_sans_rendu() {
        let mut bus = Bus::for_test_with_prg(&[0xA9]);
        let mut ppu = Ppu::new(); // mask = 0 -> pas de saut, meme en impaire
        for _ in 0..(341 * 262 + 341 * 262) {
            ppu.tick(&mut (*bus.mapper));
        }
        assert_eq!(ppu.position(), (0, 0));
        assert_eq!(ppu.frame, 2);
    }
}
