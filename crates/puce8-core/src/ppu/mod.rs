//! PPU minimale : geometrie 341 x 262, VBlank en (241, 1) (E13a), registres loopy (E15a).
//! E15b : memoire PPU (tables de motifs, CIRAM, palette) et $2007.
// wiki: PPU_scrolling ; wiki: PPU_masks_and_control

pub mod registers;

use crate::mapper::{Mapper, Mirroring};
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
    /// Buffer de lecture $2007 (lecture retardee d'un cycle).
    pub read_buffer: u8,
    /// CIRAM : 4 Ko de RAM des nametables (page choisie par le mirroring).
    pub ciram: [u8; 4096],
    /// Palette : 32 octets ($3F00-$3FFF), valeurs masquees a $3F.
    pub palette: [u8; 32],
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
            read_buffer: 0,
            ciram: [0; 4096],
            palette: [0; 32],
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

    /// Lecture memoire PPU (E15b) : tables de motifs, CIRAM (mirroring), palette.
    fn ppu_read(&mut self, addr: u16, mapper: &mut dyn Mapper) -> u8 {
        let a = addr & 0x3FFF;
        if a < 0x2000 {
            mapper.ppu_read(a)
        } else if a < 0x3F00 {
            // Nametable : $2000-$2EFF (+ miroir $3000-$3EFF), page choisie par le mirroring.
            let n = ((a >> 10) & 3) as usize;
            self.ciram[self.ciram_page(n, mapper) * 1024 + (a & 0x3FF) as usize]
        } else {
            // Palette : $3F10/$3F14/$3F18/$3F1C sont des miroirs de $3F00/$3F04/$3F08/$3F0C.
            self.palette[Self::palette_index(a)]
        }
    }

    /// Ecriture memoire PPU (E15b) : tables de motifs, CIRAM, palette ($3F).
    fn ppu_write(&mut self, addr: u16, value: u8, mapper: &mut dyn Mapper) {
        let a = addr & 0x3FFF;
        if a < 0x2000 {
            mapper.ppu_write(a, value);
        } else if a < 0x3F00 {
            let n = ((a >> 10) & 3) as usize;
            self.ciram[self.ciram_page(n, mapper) * 1024 + (a & 0x3FF) as usize] = value;
        } else {
            self.palette[Self::palette_index(a)] = value & 0x3F;
        }
    }

    /// Page CIRAM du nametable n : relit le mirroring a chaque acces.
    fn ciram_page(&self, n: usize, mapper: &dyn Mapper) -> usize {
        match mapper.mirroring() {
            Mirroring::Horizontal => [0, 0, 1, 1][n],
            Mirroring::Vertical => [0, 1, 0, 1][n],
            Mirroring::SingleScreenLower => 0,
            Mirroring::SingleScreenUpper => 1,
            Mirroring::FourScreen => n,
        }
    }

    /// Index palette d'une adresse $3Fxx (miroirs $3F10/$3F14/$3F18/$3F1C).
    fn palette_index(addr: u16) -> usize {
        match addr & 0x1F {
            16 => 0,
            20 => 4,
            24 => 8,
            28 => 12,
            p => p as usize,
        }
    }

    /// Lecture CPU d'un registre PPU (reg = addr & 7). $2002 efface VBlank et `w`.
    pub fn cpu_read_register(&mut self, reg: u16, mapper: &mut dyn Mapper) -> u8 {
        let r = usize::from(reg & 0x07);
        if r == 7 {
            // $2007 : renvoie le buffer de lecture (ou palette | open bus), puis charge ppu_read(v).
            let v = self.regs.v;
            let value = if v < 0x3F00 {
                self.read_buffer
            } else {
                self.palette[Self::palette_index(v)] | (self.regs.io_latch & 0xC0)
            };
            // Une lecture palette charge aussi ppu_read(v - $1000) dans le buffer.
            let next = if v >= 0x3F00 { v - 0x1000 } else { v };
            self.read_buffer = self.ppu_read(next, mapper);
            self.regs.incr_v();
            mapper.notify_ppu_address(self.regs.v);
            return value;
        }
        let value = self.regs.read(r as u8, self.vblank);
        if r == 2 {
            self.vblank = false; // $2002 efface VBlank
        }
        value
    }

    /// Lecture sans effet : meme valeur que `cpu_read_register`, sans effacer.
    pub fn cpu_peek_register(&self, reg: u16) -> u8 {
        let r = usize::from(reg & 0x07);
        if r == 7 {
            // $2007 : buffer de lecture (ou palette | open bus), sans effet de bord.
            let v = self.regs.v;
            return if v < 0x3F00 {
                self.read_buffer
            } else {
                self.palette[Self::palette_index(v)] | (self.regs.io_latch & 0xC0)
            };
        }
        self.regs.peek(r as u8, self.vblank)
    }

    /// Ecriture CPU d'un registre PPU (reg = addr & 7).
    pub fn cpu_write_register(&mut self, reg: u16, v: u8, mapper: &mut dyn Mapper) {
        let r = usize::from(reg & 0x07);
        if r == 7 {
            // $2007 : ecrit la memoire PPU a l'adresse v (io_latch mis a jour), puis increment.
            self.ppu_write(self.regs.v & 0x3FFF, v, mapper);
            self.regs.write(7, v);
            self.regs.incr_v();
            mapper.notify_ppu_address(self.regs.v);
            return;
        }
        // Deuxieme ecriture $2006 : notifier le mapper avec la nouvelle adresse v.
        let second_2006 = r == 6 && self.regs.w;
        self.regs.write(r as u8, v);
        if second_2006 {
            mapper.notify_ppu_address(self.regs.v);
        }
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
    use crate::mapper::Mirroring;

    /// Mapper de test : memorise la derniere adresse PPU notifiee.
    struct MapperNotif {
        last: Option<u16>,
    }

    impl Mapper for MapperNotif {
        fn cpu_read(&mut self, _addr: u16) -> Option<u8> {
            None
        }
        fn cpu_peek(&self, _addr: u16) -> Option<u8> {
            None
        }
        fn cpu_write(&mut self, _addr: u16, _value: u8) {}
        fn ppu_read(&mut self, _addr: u16) -> u8 {
            0
        }
        fn ppu_write(&mut self, _addr: u16, _value: u8) {}
        fn mirroring(&self) -> Mirroring {
            Mirroring::Horizontal
        }
        fn notify_ppu_address(&mut self, addr: u16) {
            self.last = Some(addr);
        }
    }

    /// Mapper de test : 8 Ko de RAM tables de motifs + mirroring configurable.
    struct MapperMem {
        mem: [u8; 0x2000],
        mir: Mirroring,
        last: Option<u16>,
    }

    impl Mapper for MapperMem {
        fn cpu_read(&mut self, _addr: u16) -> Option<u8> {
            None
        }
        fn cpu_peek(&self, _addr: u16) -> Option<u8> {
            None
        }
        fn cpu_write(&mut self, _addr: u16, _value: u8) {}
        fn ppu_read(&mut self, addr: u16) -> u8 {
            self.mem[usize::from(addr & 0x1FFF)]
        }
        fn ppu_write(&mut self, addr: u16, value: u8) {
            self.mem[usize::from(addr & 0x1FFF)] = value;
        }
        fn mirroring(&self) -> Mirroring {
            self.mir
        }
        fn notify_ppu_address(&mut self, addr: u16) {
            self.last = Some(addr);
        }
    }

    #[test]
    fn notif_2006_deuxieme_ecriture() {
        let mut ppu = Ppu::new();
        let mut m = MapperNotif { last: None };
        ppu.cpu_write_register(0x2006, 0x21, &mut m); // 1re ecriture : pas de notification
        assert_eq!(m.last, None);
        ppu.cpu_write_register(0x2006, 0x08, &mut m); // 2e ecriture : v = $2108 notifie
        assert_eq!(m.last, Some(0x2108));
    }

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

    #[test]
    fn lecture_bufferisee() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Horizontal,
            last: None,
        };
        ppu.cpu_write_register(0x2006, 0x00, &mut m);
        ppu.cpu_write_register(0x2006, 0x10, &mut m); // v = $0010
        m.mem[0x10] = 0xAB;
        assert_eq!(ppu.cpu_read_register(0x2007, &mut m), 0); // buffer ancien (vide)
        assert_eq!(ppu.read_buffer, 0xAB); // ppu_read($0010) charge le buffer
        assert_eq!(ppu.regs.v, 0x0011); // v += 1
        assert_eq!(m.last, Some(0x0011)); // notification avec la nouvelle adresse
    }

    #[test]
    fn palette_non_bufferisee() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Horizontal,
            last: None,
        };
        ppu.cpu_write_register(0x2006, 0x3F, &mut m);
        ppu.cpu_write_register(0x2006, 0x01, &mut m); // v = $3F01
        ppu.cpu_write_register(0x2007, 0x7A, &mut m);
        assert_eq!(ppu.palette[1], 0x3A); // masquee a $3F (0x7A & $3F)
                                          // Lecture palette : non bufferisee -> renvoie palette | (io_latch & $C0) directement.
        ppu.regs.v = 0x3F01;
        assert_eq!(ppu.cpu_read_register(0x2007, &mut m), 0x7A); // 0x3A | (0x7A & $C0)
                                                                 // Lecture "shadow" : read_buffer = ppu_read(v - $1000). v=$3F01 -> $2F01,
                                                                 // nametable n=3 (offset $300), Horizontal -> page 1.
        ppu.regs.v = 0x3F01;
        ppu.ciram[1024 + (0x2F01 & 0x3FF)] = 0xCD;
        ppu.cpu_read_register(0x2007, &mut m);
        assert_eq!(ppu.read_buffer, 0xCD); // shadow read charge le buffer depuis la CIRAM
    }

    #[test]
    fn palette_miroir_10() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Horizontal,
            last: None,
        };
        for (lo, dst) in [(0x10u8, 0usize), (0x14, 4), (0x18, 8), (0x1C, 12)] {
            ppu.cpu_write_register(0x2006, 0x3F, &mut m);
            ppu.cpu_write_register(0x2006, lo, &mut m); // v = $3F1x
            ppu.cpu_write_register(0x2007, 0x2A, &mut m);
            assert_eq!(ppu.palette[dst], 0x2A); // $3F1x est un miroir de $3F0x
        }
    }

    #[test]
    fn increment_32() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Horizontal,
            last: None,
        };
        ppu.regs.ctrl = 0x04; // bit 2 : increment de $32
        ppu.regs.v = 0x7FFF;
        ppu.cpu_read_register(0x2007, &mut m);
        assert_eq!(ppu.regs.v, 0x001F); // +32 avec wrap a $8000
    }

    #[test]
    fn mirroring_vertical() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Vertical,
            last: None,
        };
        ppu.cpu_write_register(0x2006, 0x20, &mut m);
        ppu.cpu_write_register(0x2006, 0x00, &mut m); // v = $2000
        ppu.cpu_write_register(0x2007, 0xAB, &mut m);
        assert_eq!(ppu.ciram[0], 0xAB); // n=0 -> page 0
        ppu.regs.v = 0x2800; // n=2 -> aussi page 0 en mirroring vertical
        ppu.cpu_read_register(0x2007, &mut m); // renvoie le buffer ancien...
        assert_eq!(ppu.read_buffer, 0xAB); // ...mais $2800 est visible a $2000
    }

    #[test]
    fn mirroring_horizontal() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Horizontal,
            last: None,
        };
        ppu.cpu_write_register(0x2006, 0x20, &mut m);
        ppu.cpu_write_register(0x2006, 0x00, &mut m); // v = $2000
        ppu.cpu_write_register(0x2007, 0xCD, &mut m);
        assert_eq!(ppu.ciram[0], 0xCD); // n=0 -> page 0
        ppu.regs.v = 0x2400; // n=1 -> aussi page 0 en mirroring horizontal
        ppu.cpu_read_register(0x2007, &mut m);
        assert_eq!(ppu.read_buffer, 0xCD); // $2400 est visible a $2000
    }

    #[test]
    fn miroir_3000() {
        let mut ppu = Ppu::new();
        let mut m = MapperMem {
            mem: [0; 0x2000],
            mir: Mirroring::Horizontal,
            last: None,
        };
        ppu.cpu_write_register(0x2006, 0x30, &mut m);
        ppu.cpu_write_register(0x2006, 0x50, &mut m); // v = $3050
        ppu.cpu_write_register(0x2007, 0xEF, &mut m);
        assert_eq!(ppu.ciram[0x50], 0xEF); // $3050 est un miroir de $2050
    }
}
