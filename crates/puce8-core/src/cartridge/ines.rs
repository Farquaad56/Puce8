//! Parsing de l'en-tete iNES / NES 2.0.
// wiki: INES (iNES file format) ; wiki: NES_2_0 (Header)

use std::fmt;

use crate::mapper::Mirroring;
use crate::region::TvSystem;

const HEADER_SIZE: usize = 16;
const TRAINER_SIZE: usize = 512;
const PRG_BANK_SIZE: usize = 16 * 1024;
const CHR_BANK_SIZE: usize = 8 * 1024;
const DEFAULT_PRG_RAM: usize = 8 * 1024;
const DEFAULT_CHR_RAM: usize = 8 * 1024;
const MAGIC: [u8; 4] = [b'N', b'E', b'S', 0x1A];

/// Erreurs de lecture d'un fichier `.nes`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RomError {
    TooShort,
    BadMagic,
    Truncated { expected: usize, got: usize },
    Unsupported(String),
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RomError::TooShort => write!(f, "fichier trop court pour un en-tete iNES"),
            RomError::BadMagic => write!(f, "signature \"NES\\x1A\" absente"),
            RomError::Truncated { expected, got } => {
                write!(f, "fichier tronque : {expected} octets attendus, {got} lus")
            }
            RomError::Unsupported(msg) => write!(f, "non supporte : {msg}"),
        }
    }
}

impl std::error::Error for RomError {}

/// Cartouche validee (sans mapper).
#[derive(Clone, Debug)]
pub struct Cartridge {
    pub mapper_id: u16,
    pub submapper: u8,
    pub prg_rom: Vec<u8>,
    /// Vide si la carte utilise de la CHR-RAM.
    pub chr_rom: Vec<u8>,
    /// 0 si CHR-ROM.
    pub chr_ram_size: usize,
    /// 8192 par defaut.
    pub prg_ram_size: usize,
    pub mirroring: Mirroring,
    pub has_battery: bool,
    pub is_nes2: bool,
    /// Systeme TV declare dans l'en-tete (Eopt2b).
    pub tv_system: TvSystem,
}

/// Taille NES 2.0 codee `64 << n` ; `n = 0` signifie "aucune".
// wiki: NES_2_0 (PRG-(NV)RAM/EEPROM)
fn shift_size(n: u8) -> usize {
    if n == 0 {
        0
    } else {
        64usize << n
    }
}

impl Cartridge {
    pub fn from_bytes(data: &[u8]) -> Result<Self, RomError> {
        let header: &[u8] = data.get(..HEADER_SIZE).ok_or(RomError::TooShort)?;
        if header[0..4] != MAGIC {
            return Err(RomError::BadMagic);
        }

        let flags6 = header[6];
        let flags7 = header[7];

        // wiki: NES_2_0 (Identification) : bits 2-3 de l'octet 7 = 10
        let is_nes2 = (flags7 & 0x0C) == 0x08;

        // wiki: INES (Variant comparison) : en-tete "sale" (ex. "DiskDude!")
        let dirty = !is_nes2 && header[12..16].iter().any(|&b| b != 0);

        let has_trainer = flags6 & 0x04 != 0;
        let has_battery = flags6 & 0x02 != 0;
        let mirroring = if flags6 & 0x08 != 0 {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let (mapper_id, submapper, prg_banks, chr_banks) = if is_nes2 {
            let size_msb = header[9];
            let prg_msb = size_msb & 0x0F;
            let chr_msb = size_msb >> 4;
            // wiki: NES_2_0 (PRG-ROM Area) : MSB = 0xF -> notation exposant-multiplicateur
            if prg_msb == 0x0F || chr_msb == 0x0F {
                return Err(RomError::Unsupported(
                    "taille ROM NES 2.0 au format exposant-multiplicateur".to_string(),
                ));
            }
            let mapper = u16::from(flags6 >> 4)
                | u16::from(flags7 & 0xF0)
                | (u16::from(header[8] & 0x0F) << 8);
            let prg = (usize::from(prg_msb) << 8) | usize::from(header[4]);
            let chr = (usize::from(chr_msb) << 8) | usize::from(header[5]);
            (mapper, header[8] >> 4, prg, chr)
        } else {
            let mapper = if dirty {
                u16::from(flags6 >> 4)
            } else {
                u16::from(flags6 >> 4) | u16::from(flags7 & 0xF0)
            };
            (mapper, 0, usize::from(header[4]), usize::from(header[5]))
        };

        // wiki: NES_2_0 (Header) : octet 12 bits 0-1 ; wiki: INES : octet 9 bit 0, octet 10 bits 0-1.
        let tv_system = if is_nes2 {
            match header[12] & 0x03 {
                0 => TvSystem::Ntsc,
                1 => TvSystem::Pal,
                2 => TvSystem::Multi,
                _ => TvSystem::Dendy,
            }
        } else if !dirty && (header[9] & 0x01 != 0 || header[10] & 0x03 == 0x02) {
            TvSystem::Pal
        } else {
            TvSystem::Inconnu
        };

        let prg_size = prg_banks * PRG_BANK_SIZE;
        let chr_size = chr_banks * CHR_BANK_SIZE;
        let trainer_size = if has_trainer { TRAINER_SIZE } else { 0 };

        // Ordre : en-tete -> trainer -> PRG-ROM -> CHR-ROM -> reste ignore.
        let prg_start = HEADER_SIZE + trainer_size;
        let chr_start = prg_start + prg_size;
        let expected = chr_start + chr_size;
        if data.len() < expected {
            return Err(RomError::Truncated {
                expected,
                got: data.len(),
            });
        }
        let prg_rom = data
            .get(prg_start..chr_start)
            .ok_or(RomError::Truncated {
                expected,
                got: data.len(),
            })?
            .to_vec();
        let chr_rom = data
            .get(chr_start..expected)
            .ok_or(RomError::Truncated {
                expected,
                got: data.len(),
            })?
            .to_vec();

        let prg_ram_size = if is_nes2 && header[10] != 0 {
            let volatile = shift_size(header[10] & 0x0F);
            let nonvolatile = shift_size(header[10] >> 4);
            volatile.max(nonvolatile)
        } else {
            DEFAULT_PRG_RAM
        };

        let chr_ram_size = if chr_size != 0 {
            0
        } else if is_nes2 {
            shift_size(header[11] & 0x0F).max(DEFAULT_CHR_RAM)
        } else {
            DEFAULT_CHR_RAM
        };

        Ok(Cartridge {
            mapper_id,
            submapper,
            prg_rom,
            chr_rom,
            chr_ram_size,
            prg_ram_size,
            mirroring,
            has_battery,
            is_nes2,
            tv_system,
        })
    }

    /// Resume sur une ligne, ex. `mapper=0 prg=16K chr=8K mir=V bat=0 nes2=0 tv=NTSC`.
    pub fn summary(&self) -> String {
        let chr = if self.chr_rom.is_empty() {
            format!("ram{}K", self.chr_ram_size / 1024)
        } else {
            format!("{}K", self.chr_rom.len() / 1024)
        };
        let mir = match self.mirroring {
            Mirroring::Horizontal => "H",
            Mirroring::Vertical => "V",
            Mirroring::SingleScreenLower => "1L",
            Mirroring::SingleScreenUpper => "1U",
            Mirroring::FourScreen => "4",
        };
        format!(
            "mapper={} prg={}K chr={} mir={} bat={} nes2={} tv={}",
            self.mapper_id,
            self.prg_rom.len() / 1024,
            chr,
            mir,
            u8::from(self.has_battery),
            u8::from(self.is_nes2),
            self.tv_system.region().label()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM synthetique : PRG = `i % 256`, CHR = `0xC0 | (i % 64)`.
    /// Si `flags6` a le bit trainer, 512 octets de 0xEE sont inseres avant la PRG.
    fn make_rom(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut rom = vec![b'N', b'E', b'S', 0x1A, prg_banks, chr_banks, flags6, flags7];
        rom.extend_from_slice(&[0; 8]);
        if flags6 & 0x04 != 0 {
            rom.resize(rom.len() + TRAINER_SIZE, 0xEE);
        }
        let prg_len = usize::from(prg_banks) * PRG_BANK_SIZE;
        rom.extend((0..prg_len).map(|i| (i % 256) as u8));
        let chr_len = usize::from(chr_banks) * CHR_BANK_SIZE;
        rom.extend((0..chr_len).map(|i| 0xC0 | (i % 64) as u8));
        rom
    }

    #[test]
    fn magic_invalide() {
        let mut rom = make_rom(1, 1, 0, 0);
        rom[2] = b'Z';
        assert_eq!(Cartridge::from_bytes(&rom).unwrap_err(), RomError::BadMagic);
    }

    #[test]
    fn trop_court() {
        let rom = [b'N', b'E', b'S', 0x1A, 1, 1, 0, 0, 0, 0];
        assert_eq!(Cartridge::from_bytes(&rom).unwrap_err(), RomError::TooShort);
    }

    #[test]
    fn nrom_16k_vertical() {
        let cart = Cartridge::from_bytes(&make_rom(1, 1, 0x01, 0)).unwrap();
        assert_eq!(cart.mapper_id, 0);
        assert_eq!(cart.prg_rom.len(), 16_384);
        assert_eq!(cart.chr_rom.len(), 8_192);
        assert_eq!(cart.chr_ram_size, 0);
        assert_eq!(cart.prg_ram_size, 8_192);
        assert_eq!(cart.mirroring, Mirroring::Vertical);
        assert_eq!(cart.prg_rom[255], 255);
        assert_eq!(cart.chr_rom[65], 0xC1);
        assert_eq!(
            cart.summary(),
            "mapper=0 prg=16K chr=8K mir=V bat=0 nes2=0 tv=NTSC"
        );
    }

    #[test]
    fn mapper_4_quartets() {
        let cart = Cartridge::from_bytes(&make_rom(1, 1, 0x40, 0x00)).unwrap();
        assert_eq!(cart.mapper_id, 4);
        assert_eq!(cart.mirroring, Mirroring::Horizontal);
    }

    #[test]
    fn mapper_quartet_haut() {
        let cart = Cartridge::from_bytes(&make_rom(1, 1, 0x10, 0x40)).unwrap();
        assert_eq!(cart.mapper_id, 0x41);
    }

    #[test]
    fn chr_ram() {
        let cart = Cartridge::from_bytes(&make_rom(1, 0, 0, 0)).unwrap();
        assert!(cart.chr_rom.is_empty());
        assert_eq!(cart.chr_ram_size, 8_192);
    }

    #[test]
    fn trainer_saute() {
        let cart = Cartridge::from_bytes(&make_rom(1, 1, 0x04, 0)).unwrap();
        assert_eq!(cart.prg_rom[0], 0);
        assert_eq!(cart.prg_rom[1], 1);
        assert_eq!(cart.chr_rom[0], 0xC0);
    }

    #[test]
    fn tronque() {
        let mut rom = make_rom(1, 0, 0, 0);
        rom[4] = 2;
        assert_eq!(
            Cartridge::from_bytes(&rom).unwrap_err(),
            RomError::Truncated {
                expected: 16 + 2 * 16_384,
                got: 16 + 16_384
            }
        );
    }

    #[test]
    fn diskdude() {
        let mut rom = make_rom(1, 1, 0x10, 0);
        rom[7..16].copy_from_slice(b"DiskDude!");
        let cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.mapper_id, 1);
        assert!(!cart.is_nes2);
    }

    #[test]
    fn nes2_detecte() {
        let mut rom = make_rom(1, 1, 0, 0x08);
        rom[8] = 0x00;
        let cart = Cartridge::from_bytes(&rom).unwrap();
        assert!(cart.is_nes2);
        assert_eq!(cart.mapper_id, 0);
        assert_eq!(cart.submapper, 0);
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
        assert_eq!(cart.mapper_id, 0);
        assert_eq!(cart.prg_rom.len(), 16_384);
        assert_eq!(cart.chr_rom.len(), 8_192);
    }

    // Complement (hors liste des 11) : regles NES 2.0 de la specification.
    #[test]
    fn nes2_exposant_refuse() {
        let mut rom = make_rom(1, 1, 0, 0x08);
        rom[9] = 0xF0;
        assert!(matches!(
            Cartridge::from_bytes(&rom),
            Err(RomError::Unsupported(_))
        ));
    }

    #[test]
    fn nes2_mapper_sous_mapper_et_ram() {
        let mut rom = make_rom(1, 0, 0x12, 0x48);
        rom[8] = 0x31; // sous-mapper 3, mapper bits 8-11 = 1
        rom[10] = 0x70; // PRG-NVRAM = 64 << 7 = 8 Ko
        rom[11] = 0x09; // CHR-RAM = 64 << 9 = 32 Ko
        let cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.mapper_id, 0x141);
        assert_eq!(cart.submapper, 3);
        assert!(cart.has_battery);
        assert_eq!(cart.prg_ram_size, 8_192);
        assert_eq!(cart.chr_ram_size, 32_768);
    }
}
