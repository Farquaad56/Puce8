//! Logique pure du frontend (E23a1) : sans egui, testable.

use puce8_core::nes::Nes;
use puce8_core::ppu::palette::to_rgba;

/// Duree d'une image NTSC en secondes (60,0988 images/s).
pub const FRAME_DT: f64 = 1.0 / 60.0988;
/// Nombre maximum d'images rattrapees par rafraichissement (pas de spirale).
pub const MAX_CATCH_UP: u32 = 4;
/// Images par rafraichissement en avance rapide (Tab).
pub const TURBO_FRAMES: u32 = 8;

/// Options de la ligne de commande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub rom: Option<String>,
    pub scale: u32,
    pub headless_frames: Option<u32>,
}

/// `puce8-desktop [rom.nes] [--scale N] [--headless-frames N]`.
pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        rom: None,
        scale: 3,
        headless_frames: None,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" | "--headless-frames" => {
                let flag = args[i].clone();
                let n: u32 = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("{flag} attend un nombre"))?;
                if flag == "--scale" {
                    out.scale = n.clamp(1, 5);
                } else {
                    out.headless_frames = Some(n);
                }
                i += 2;
            }
            a if a.starts_with("--") => return Err(format!("option inconnue : {a}")),
            a => {
                out.rom = Some(a.to_string());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Charge une ROM ; message d'erreur lisible si le fichier ou la ROM est invalide.
pub fn load_rom(path: &str) -> Result<Nes, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("impossible de lire {path} : {e}"))?;
    Nes::from_rom(&bytes).map_err(|e| format!("ROM invalide {path} : {e:?}"))
}

/// Texte de la barre d'etat pour une ROM : nom du fichier + numero de mapper (E23a3).
pub fn rom_label(path: &str) -> String {
    let nom = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    match std::fs::read(path)
        .ok()
        .and_then(|b| puce8_core::cartridge::Cartridge::from_bytes(&b).ok())
    {
        Some(c) => format!("{nom} - mapper {}", c.mapper_id),
        None => nom,
    }
}

/// Accumulateur de cadence : ajoute `dt` et renvoie le nombre d'images a emuler.
/// Normal : 0 a MAX_CATCH_UP ; au-dela, le retard est abandonne (acc = 0).
/// Turbo : TURBO_FRAMES, sans limite de temps (acc = 0).
pub fn frames_to_run(acc: &mut f64, dt: f64, turbo: bool) -> u32 {
    if turbo {
        *acc = 0.0;
        return TURBO_FRAMES;
    }
    *acc += dt;
    let mut n = 0;
    while *acc >= FRAME_DT && n < MAX_CATCH_UP {
        *acc -= FRAME_DT;
        n += 1;
    }
    if *acc >= FRAME_DT {
        *acc = 0.0; // trop de retard : on abandonne (pas de spirale)
    }
    n
}

/// Framebuffer (indices palette) -> octets RGBA (4 par pixel, alpha = 255).
pub fn rgba_bytes(fb: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(fb.len() * 4);
    for &px in fb {
        let c = to_rgba(px);
        out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 0xFF]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn accumulateur_1_image() {
        let mut acc = 0.0;
        assert_eq!(frames_to_run(&mut acc, 1.0 / 60.0, false), 1);
        assert!(acc < FRAME_DT);
    }

    #[test]
    fn accumulateur_rien_si_trop_tot() {
        let mut acc = 0.0;
        assert_eq!(frames_to_run(&mut acc, 0.005, false), 0);
        assert_eq!(frames_to_run(&mut acc, 0.012, false), 1); // 5 + 12 ms > 16,6 ms
    }

    #[test]
    fn accumulateur_plafond() {
        let mut acc = 0.0;
        assert_eq!(frames_to_run(&mut acc, 1.0, false), 4);
        assert_eq!(acc, 0.0);
    }

    #[test]
    fn accumulateur_turbo() {
        let mut acc = 0.5;
        assert_eq!(frames_to_run(&mut acc, 0.001, true), 8);
        assert_eq!(acc, 0.0);
    }

    #[test]
    fn rgba_taille() {
        let fb = vec![0x30u16; 256 * 240];
        let b = rgba_bytes(&fb);
        assert_eq!(b.len(), 256 * 240 * 4);
        assert_eq!(&b[0..4], &[0xEC, 0xEE, 0xEC, 0xFF]); // $30 = blanc
    }

    #[test]
    fn arguments() {
        let a = parse_args(&s(&["jeu.nes", "--scale", "2"])).unwrap();
        assert_eq!(a.rom.as_deref(), Some("jeu.nes"));
        assert_eq!(a.scale, 2);
        assert_eq!(a.headless_frames, None);
        let a = parse_args(&s(&["--headless-frames", "600", "x.nes"])).unwrap();
        assert_eq!(a.headless_frames, Some(600));
        assert_eq!(parse_args(&s(&[])).unwrap().scale, 3);
        assert!(parse_args(&s(&["--scale"])).is_err());
        assert!(parse_args(&s(&["--vite"])).is_err());
    }

    #[test]
    fn rom_invalide() {
        assert!(load_rom("n_existe_pas.nes").is_err());
    }

    #[test]
    fn etiquette_rom_absente() {
        assert_eq!(rom_label("dossier/n_existe_pas.nes"), "n_existe_pas.nes");
    }
}
