//! Script d'entrees pour le CLI (E24b3) : `"60:START;70:;200:A,RIGHT"`.
//! A partir de l'image indiquee, les boutons listes sont maintenus ; une liste vide relache tout.
//! Noms : A B SELECT START UP DOWN LEFT RIGHT (majuscules ou minuscules).

use puce8_core::controller::{
    BTN_A, BTN_B, BTN_DOWN, BTN_LEFT, BTN_RIGHT, BTN_SELECT, BTN_START, BTN_UP,
};
use puce8_core::nes::Nes;

/// Script trie par image croissante : (image de depart, boutons de la manette 1).
pub type InputScript = Vec<(u32, u8)>;

fn bouton(nom: &str) -> Result<u8, String> {
    Ok(match nom.trim().to_ascii_uppercase().as_str() {
        "A" => BTN_A,
        "B" => BTN_B,
        "SELECT" => BTN_SELECT,
        "START" => BTN_START,
        "UP" => BTN_UP,
        "DOWN" => BTN_DOWN,
        "LEFT" => BTN_LEFT,
        "RIGHT" => BTN_RIGHT,
        autre => return Err(format!("bouton inconnu : {autre}")),
    })
}

/// Analyse `"image:BOUTON,BOUTON;image:;..."`.
pub fn parse_input_script(s: &str) -> Result<InputScript, String> {
    let mut out = Vec::new();
    for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (img, liste) = part
            .split_once(':')
            .ok_or_else(|| format!("entree sans ':' : {part}"))?;
        let img: u32 = img
            .trim()
            .parse()
            .map_err(|_| format!("image invalide : {img}"))?;
        let mut b = 0;
        for nom in liste.split(',').filter(|n| !n.trim().is_empty()) {
            b |= bouton(nom)?;
        }
        out.push((img, b));
    }
    out.sort_by_key(|&(img, _)| img);
    Ok(out)
}

/// Boutons maintenus pendant l'image `frame` (0 = premiere image jouee).
pub fn buttons_at(script: &[(u32, u8)], frame: u32) -> u8 {
    script
        .iter()
        .take_while(|&&(img, _)| img <= frame)
        .last()
        .map_or(0, |&(_, b)| b)
}

/// Joue `frames` images en appliquant le script a la manette 1 avant chaque image.
pub fn run_frames(nes: &mut Nes, frames: u32, script: &[(u32, u8)]) {
    for f in 0..frames {
        nes.set_buttons(0, buttons_at(script, f));
        nes.run_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_input_script_ok() {
        let s = parse_input_script("60:START;70:;200:A,right").unwrap();
        assert_eq!(s, vec![(60, BTN_START), (70, 0), (200, BTN_A | BTN_RIGHT)]);
        assert!(parse_input_script("10:SAUTER").is_err());
        assert!(parse_input_script("dix:A").is_err());
        assert_eq!(parse_input_script("").unwrap(), vec![]);
    }

    #[test]
    fn boutons_a_l_image() {
        let s = parse_input_script("60:START;70:;200:A").unwrap();
        assert_eq!(buttons_at(&s, 0), 0);
        assert_eq!(buttons_at(&s, 60), BTN_START);
        assert_eq!(buttons_at(&s, 69), BTN_START);
        assert_eq!(buttons_at(&s, 70), 0);
        assert_eq!(buttons_at(&s, 500), BTN_A);
    }

    /// ROM synthetique : lit les 8 boutons de la manette 1 en boucle (dans $11), puis les recopie
    /// d'un coup en $0010 (sinon on pourrait lire $0010 a moitie construit en fin d'image).
    fn rom_lecture_manette() -> Vec<u8> {
        let code = [
            0xA9, 0x01, 0x8D, 0x16, 0x40, // LDA #1 ; STA $4016 (strobe haut)
            0xA9, 0x00, 0x8D, 0x16, 0x40, // LDA #0 ; STA $4016 (verrouillage)
            0xA2, 0x08, // LDX #8
            0xAD, 0x16, 0x40, // boucle : LDA $4016
            0x4A, // LSR A (bit 0 -> C)
            0x66, 0x11, // ROR $11 (C -> bit 7 de $11)
            0xCA, // DEX
            0xD0, 0xF7, // BNE boucle
            0xA5, 0x11, 0x85, 0x10, // LDA $11 ; STA $10
            0x4C, 0x00, 0x80, // JMP $8000
        ];
        let mut rom = vec![0x4E, 0x45, 0x53, 0x1A, 0x01, 0x00];
        rom.extend_from_slice(&[0; 10]);
        let mut prg = vec![0xEA; 16384];
        prg[..code.len()].copy_from_slice(&code);
        prg[0x3FFC..0x3FFE].copy_from_slice(&[0x00, 0x80]);
        rom.extend(prg);
        rom
    }

    #[test]
    fn script_rom_synthetique() {
        let mut nes = Nes::from_rom(&rom_lecture_manette()).unwrap();
        let script = parse_input_script("5:A,LEFT").unwrap();
        run_frames(&mut nes, 4, &script);
        assert_eq!(nes.peek(0x0010), 0x00); // images 0-3 : rien d'appuye
        let mut nes = Nes::from_rom(&rom_lecture_manette()).unwrap();
        run_frames(&mut nes, 10, &script);
        assert_eq!(nes.peek(0x0010), 0x41); // A (bit 0) + LEFT (bit 6)
    }
}
