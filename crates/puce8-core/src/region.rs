//! Region NTSC / PAL : parametres d'affichage et detection automatique (Eopt2b).
// wiki: NES_2_0 (Header) ; wiki: INES (iNES file format)

use crate::cartridge::Cartridge;

/// Region video de la console. Le defaut est NTSC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Region {
    #[default]
    Ntsc,
    Pal,
}

impl Region {
    /// "NTSC" ou "PAL".
    pub fn label(&self) -> &'static str {
        match self {
            Region::Ntsc => "NTSC",
            Region::Pal => "PAL",
        }
    }

    /// Frequence CPU en Hz.
    pub fn cpu_hz(&self) -> u32 {
        match self {
            Region::Ntsc => 1_789_773,
            Region::Pal => 1_662_607,
        }
    }

    /// Images par seconde (moyenne).
    pub fn frame_hz(&self) -> f64 {
        match self {
            Region::Ntsc => 60.0988,
            Region::Pal => 50.0070,
        }
    }

    /// Lignes par image (NTSC : 0-261, PAL : 0-311).
    pub fn scanlines(&self) -> usize {
        match self {
            Region::Ntsc => 262,
            Region::Pal => 312,
        }
    }

    /// Ligne pre-render (la derniere de l'image).
    pub fn pre_render_line(&self) -> usize {
        match self {
            Region::Ntsc => 261,
            Region::Pal => 311,
        }
    }

    /// Saut du point impair a l'affichage (NTSC uniquement).
    pub fn odd_frame_skip(&self) -> bool {
        match self {
            Region::Ntsc => true,
            Region::Pal => false,
        }
    }

    /// Detection : force manuelle > en-tete > nom de fichier > defaut NTSC.
    pub fn detect(cart: &Cartridge, nom: &str, force: Option<Region>) -> (Region, RegionSource) {
        if let Some(region) = force {
            return (region, RegionSource::Manuel);
        }
        match cart.tv_system {
            TvSystem::Ntsc => (Region::Ntsc, RegionSource::EnTete),
            TvSystem::Pal => (Region::Pal, RegionSource::EnTete),
            // Multi-region et Dendy : emules comme NTSC.
            TvSystem::Multi | TvSystem::Dendy => (Region::Ntsc, RegionSource::EnTete),
            TvSystem::Inconnu => {
                if nom_indique_pal(nom) {
                    (Region::Pal, RegionSource::NomFichier)
                } else {
                    (Region::Ntsc, RegionSource::Defaut)
                }
            }
        }
    }
}

/// Systeme TV declare dans l'en-tete de la cartouche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TvSystem {
    Ntsc,
    Pal,
    /// Multi-region (NES 2.0) : emule comme NTSC.
    Multi,
    /// Dendy (hors perimetre) : emule comme NTSC.
    Dendy,
    /// En-tete iNES sans indication fiable.
    Inconnu,
}

impl TvSystem {
    /// Region d'emulation associee (tout sauf Pal est emule en NTSC).
    pub fn region(&self) -> Region {
        match self {
            TvSystem::Pal => Region::Pal,
            _ => Region::Ntsc,
        }
    }
}

/// Origine de la decision de region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSource {
    /// Choix manuel (option `--region`).
    Manuel,
    /// En-tete de la cartouche (NES 2.0 ou iNES).
    EnTete,
    /// Mot du nom de fichier.
    NomFichier,
    /// Defaut NTSC.
    Defaut,
}

/// Mots du nom de fichier qui indiquent une ROM PAL (sans casse).
const PAL_WORDS: &[&str] = &[
    "pal",
    "e",
    "europe",
    "australia",
    "germany",
    "france",
    "spain",
    "italy",
    "sweden",
];

/// `true` si un mot du nom (decoupe sur tout caractere non alphanumerique) vaut un mot PAL.
fn nom_indique_pal(nom: &str) -> bool {
    nom.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|mot| PAL_WORDS.iter().any(|w| mot.eq_ignore_ascii_case(w)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM synthetique : en-tete iNES propre, PRG = `i % 256`, CHR = `0xC0 | (i % 64)`.
    fn make_rom(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut rom = vec![b'N', b'E', b'S', 0x1A, prg_banks, chr_banks, flags6, flags7];
        rom.extend_from_slice(&[0; 8]); // octets 8-15 de l'en-tete (propres)
        let prg_len = usize::from(prg_banks) * 16_384;
        rom.extend((0..prg_len).map(|i| (i % 256) as u8));
        let chr_len = usize::from(chr_banks) * 8_192;
        rom.extend((0..chr_len).map(|i| 0xC0 | (i % 64) as u8));
        rom
    }

    fn cart(rom: Vec<u8>) -> Cartridge {
        Cartridge::from_bytes(&rom).unwrap()
    }

    #[test]
    fn parametres() {
        assert_eq!(Region::Ntsc.label(), "NTSC");
        assert_eq!(Region::Pal.label(), "PAL");
        assert_eq!(Region::default(), Region::Ntsc);
        assert_eq!(Region::Ntsc.cpu_hz(), 1_789_773);
        assert_eq!(Region::Pal.cpu_hz(), 1_662_607);
        assert!((Region::Ntsc.frame_hz() - 60.0988).abs() < 1e-4);
        assert!((Region::Pal.frame_hz() - 50.0070).abs() < 1e-4);
        assert_eq!(Region::Ntsc.scanlines(), 262);
        assert_eq!(Region::Pal.scanlines(), 312);
        assert_eq!(Region::Ntsc.pre_render_line(), 261);
        assert_eq!(Region::Pal.pre_render_line(), 311);
        assert!(Region::Ntsc.odd_frame_skip());
        assert!(!Region::Pal.odd_frame_skip());
    }

    #[test]
    fn nes2_tv_system() {
        // wiki: NES_2_0 (Header) : octet 12 bits 0-1.
        let cas = [
            (0u8, TvSystem::Ntsc, Region::Ntsc),
            (1, TvSystem::Pal, Region::Pal),
            (2, TvSystem::Multi, Region::Ntsc),
            (3, TvSystem::Dendy, Region::Ntsc),
        ];
        for (octet, tv, region) in cas {
            let mut rom = make_rom(1, 1, 0, 0x08); // NES 2.0
            rom[12] = octet;
            let c = cart(rom);
            assert_eq!(c.tv_system, tv, "octet 12 = {octet}");
            assert_eq!(
                Region::detect(&c, "Game.nes", None).0,
                region,
                "octet 12 = {octet}"
            );
        }
    }

    #[test]
    fn ines_pal_octet9_bit0() {
        let mut rom = make_rom(1, 1, 0, 0);
        rom[9] = 0x01;
        let c = cart(rom);
        assert_eq!(c.tv_system, TvSystem::Pal);
        assert_eq!(
            Region::detect(&c, "Game.nes", None),
            (Region::Pal, RegionSource::EnTete)
        );
    }

    #[test]
    fn ines_pal_octet10() {
        let mut rom = make_rom(1, 1, 0, 0);
        rom[10] = 0x02; // bits 0-1 = 2
        let c = cart(rom);
        assert_eq!(c.tv_system, TvSystem::Pal);
    }

    #[test]
    fn ines_tous_zero_inconnu() {
        let c = cart(make_rom(1, 1, 0, 0));
        assert_eq!(c.tv_system, TvSystem::Inconnu);
        assert_eq!(
            Region::detect(&c, "Game.nes", None),
            (Region::Ntsc, RegionSource::Defaut)
        );
    }

    #[test]
    fn ines_sale_tv_non_lu() {
        // En-tete "sale" (cas DiskDude!) : octets 9-15 non fiables.
        let mut rom = make_rom(1, 1, 0x10, 0);
        rom[7..16].copy_from_slice(b"DiskDude!");
        let c = cart(rom);
        assert_eq!(c.tv_system, TvSystem::Inconnu);
    }

    #[test]
    fn nom_fichier_pal() {
        for nom in [
            "Game (Europe).nes",
            "Game (E).nes",
            "demo_pal.nes",
            "nes15-PAL.nes",
        ] {
            let c = cart(make_rom(1, 1, 0, 0));
            assert_eq!(
                Region::detect(&c, nom, None),
                (Region::Pal, RegionSource::NomFichier),
                "{nom}"
            );
        }
    }

    #[test]
    fn nom_fichier_ntsc() {
        for nom in [
            "full_palette.nes",
            "Game (USA).nes",
            "Game (J).nes",
            "palace.nes",
        ] {
            let c = cart(make_rom(1, 1, 0, 0));
            assert_eq!(
                Region::detect(&c, nom, None),
                (Region::Ntsc, RegionSource::Defaut),
                "{nom}"
            );
        }
    }

    #[test]
    fn priorite() {
        let mut rom = make_rom(1, 1, 0, 0);
        rom[9] = 0x01; // en-tete PAL
        let c = cart(rom);
        // La force gagne sur l'en-tete.
        assert_eq!(
            Region::detect(&c, "Game (Europe).nes", Some(Region::Ntsc)),
            (Region::Ntsc, RegionSource::Manuel)
        );
        // L'en-tete gagne sur le nom.
        assert_eq!(
            Region::detect(&c, "Game.nes", None),
            (Region::Pal, RegionSource::EnTete)
        );
    }
}
