//! Sauvegardes batterie (.sav) (E29a2) : chemin, lecture, ecriture atomique. Sans egui, testable.
//! Le .sav est ecrit a cote de la ROM par le PROGRAMME (les tests ecrivent dans `out/`).

use std::path::{Path, PathBuf};

/// `jeu.nes` -> `jeu.sav` (meme dossier que la ROM).
pub fn sav_path(rom: &str) -> PathBuf {
    Path::new(rom).with_extension("sav")
}

/// Contenu du .sav s'il existe ET a exactement la taille attendue ; sinon None.
pub fn load_sav(path: &Path, taille: usize) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    (data.len() == taille).then_some(data)
}

/// Ecriture atomique : `<nom>.tmp`, puis renommage vers `path` (remplace l'ancien fichier).
pub fn write_sav(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dossier `out/` a la racine du workspace (cree si besoin).
    fn out_dir() -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../out");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn chemin_sav() {
        assert_eq!(sav_path("roms/zelda.nes"), PathBuf::from("roms/zelda.sav"));
        assert_eq!(sav_path("jeu"), PathBuf::from("jeu.sav"));
    }

    #[test]
    fn sav_roundtrip() {
        let p = out_dir().join("test_e29_roundtrip.sav");
        let data: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        write_sav(&p, &data).unwrap();
        assert_eq!(load_sav(&p, 8192), Some(data));
        assert!(!p.with_extension("tmp").exists()); // le .tmp a ete renomme
        write_sav(&p, &[7; 8192]).unwrap(); // remplace l'ancien fichier
        assert_eq!(load_sav(&p, 8192), Some(vec![7; 8192]));
        assert_eq!(load_sav(&p, 4096), None); // mauvaise taille : ignore
    }

    #[test]
    fn sav_absent() {
        assert_eq!(load_sav(&out_dir().join("absent_e29.sav"), 8192), None);
    }
}
