//! Mappers de cartouche : trait commun, memoire CHR partagee, fabrique (ARCHI par. A3).
// wiki: NROM ; wiki: INES_Mapper_000

pub mod cnrom;
pub mod mmc1;
pub mod nrom;
pub mod uxrom;

use crate::cartridge::{Cartridge, RomError};

/// Organisation des nametables vue par le PPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    SingleScreenLower,
    SingleScreenUpper,
    FourScreen,
}

/// Contrat commun a tous les mappers de cartouche (ARCHI par. A3).
pub trait Mapper {
    /// CPU $4020-$FFFF. None = rien n'est branche -> open bus.
    fn cpu_read(&mut self, addr: u16) -> Option<u8>;
    /// Lecture sans effet de bord : meme valeur que `cpu_read`.
    fn cpu_peek(&self, addr: u16) -> Option<u8>;
    fn cpu_write(&mut self, addr: u16, value: u8);
    /// PPU $0000-$1FFF (tables de motifs, CHR-ROM ou CHR-RAM).
    fn ppu_read(&mut self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, value: u8);
    /// Lecture PPU $0000-$1FFF SANS effet de bord (vues de debogage, E18e1) : meme valeur que `ppu_read`.
    fn ppu_peek(&self, addr: u16) -> u8;
    /// Mirroring fixe, celui de l'en-tete.
    fn mirroring(&self) -> Mirroring;
    /// Appele pour CHAQUE adresse posee sur le bus PPU (rendu, $2006, $2007) -> compteur de lignes MMC3 (A12).
    fn notify_ppu_address(&mut self, _addr: u16) {}
    /// Appele a chaque cycle CPU (M2) par Nes::tick -> filtre A12 du MMC3, ecritures consecutives du MMC1.
    fn cpu_cycle(&mut self) {}
    /// true = IRQ mapper active (sensible au niveau).
    fn irq_pending(&self) -> bool {
        false
    }
    /// RAM alimentee par batterie a sauvegarder ; None si la carte n'en a pas.
    fn battery_ram(&self) -> Option<&[u8]> {
        None
    }
    /// Restaure un etat sauvegarde dans la RAM de batterie.
    fn load_battery_ram(&mut self, _data: &[u8]) {}
}

/// CHR-ROM ou CHR-RAM, reutilisable par tous les mappers.
pub struct ChrMemory {
    data: Vec<u8>,
    writable: bool,
}

impl ChrMemory {
    /// ROM si la carte en a une ; sinon RAM (minimum 8 Ko).
    // wiki: NROM ; wiki: NES_2_0 (CHR-(NV)RAM/EEPROM)
    pub fn from_cart(cart: &Cartridge) -> Self {
        if !cart.chr_rom.is_empty() {
            ChrMemory {
                data: cart.chr_rom.clone(),
                writable: false,
            }
        } else {
            let size = cart.chr_ram_size.max(8 * 1024);
            ChrMemory {
                data: vec![0; size],
                writable: true,
            }
        }
    }

    /// Lecture a `offset` (deja "banke"), repliee modulo la taille.
    pub fn read(&self, offset: usize) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        self.data[offset % self.data.len()]
    }

    /// N'ecrit que si writable (CHR-RAM).
    pub fn write(&mut self, offset: usize, value: u8) {
        if self.writable && !self.data.is_empty() {
            let idx = offset % self.data.len();
            self.data[idx] = value;
        }
    }

    /// Taille en octets.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// true si la memoire est vide.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Fabrique : mapper a partir de l'en-tete de la cartouche (NROM, UxROM, CNROM).
pub fn create_mapper(cart: Cartridge) -> Result<Box<dyn Mapper>, RomError> {
    match cart.mapper_id {
        0 => Ok(Box::new(nrom::Nrom::new(cart))),
        2 => Ok(Box::new(uxrom::Uxrom::new(cart))),
        3 => Ok(Box::new(cnrom::Cnrom::new(cart))),
        n => Err(RomError::Unsupported(format!("mapper {n}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cartouche synthetique : PRG de 16 Ko nul, CHR donne.
    fn cart(chr_rom: Vec<u8>, chr_ram_size: usize) -> Cartridge {
        Cartridge {
            mapper_id: 0,
            submapper: 0,
            prg_rom: vec![0; 16_384],
            chr_rom,
            chr_ram_size,
            prg_ram_size: 8_192,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
            is_nes2: false,
        }
    }

    #[test]
    fn chr_rom_lecture_seule() {
        let cart = cart(vec![0xC0, 0xC1], 0);
        let mut m = ChrMemory::from_cart(&cart);
        assert_eq!(m.len(), 2);
        assert_eq!(m.read(0), 0xC0);
        assert_eq!(m.read(1), 0xC1);
        m.write(0, 0xFF); // ROM -> ignore
        assert_eq!(m.read(0), 0xC0);
    }

    #[test]
    fn chr_ram_rw() {
        let cart = cart(Vec::new(), 8_192);
        let mut m = ChrMemory::from_cart(&cart);
        assert_eq!(m.len(), 8_192);
        assert_eq!(m.read(0), 0);
        m.write(5, 0x7E);
        assert_eq!(m.read(5), 0x7E);
    }

    #[test]
    fn chr_modulo() {
        let mut m = ChrMemory {
            data: (0..16).collect(),
            writable: true,
        };
        assert_eq!(m.read(m.len() + 3), m.read(3)); // 19 % 16 == 3
        m.write(m.len() + 4, 0x99); // 20 % 16 == 4
        assert_eq!(m.read(4), 0x99);
    }

    #[test]
    fn mapper_inconnu() {
        let mut rom = [0u8; 16];
        rom[0..4].copy_from_slice(&[b'N', b'E', b'S', 0x1A]);
        rom[6] = 0x30; // bits 2-5 de f6 : quartet bas du mapper (3)
        rom[7] = 0x60; // bits 4-7 de f7 : quartet haut -> mapper 99
        let cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.mapper_id, 99);
        assert!(matches!(create_mapper(cart), Err(RomError::Unsupported(_))));
    }

    #[test]
    fn vrai_nestest() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/roms/other/nestest.nes"
        );
        let Ok(data) = std::fs::read(path) else {
            eprintln!("vrai_nestest ignore : {path} absent");
            return;
        };
        let cart = Cartridge::from_bytes(&data).unwrap();
        let mut m = create_mapper(cart).expect("nestest.nes (mapper 0) doit etre creable");
        assert!(m.cpu_read(0x8000).is_some());
    }
}
