//! Configuration des manettes (E24d) : bouton NES -> source BRUTE gilrs (code du peripherique),
//! pour les manettes dont la table de correspondance de gilrs est fausse ou absente
//! (manettes USB generiques type NES/SNES : croix vue comme des boutons ou des axes inconnus).
//! Logique pure, testable ; l'assistant de configuration est dans `gui.rs`.

use puce8_core::controller::{
    BTN_A, BTN_B, BTN_DOWN, BTN_LEFT, BTN_RIGHT, BTN_SELECT, BTN_START, BTN_UP,
};

/// Les 8 boutons NES, dans l'ordre de l'assistant.
pub const NES_BUTTONS: [(u8, &str); 8] = [
    (BTN_A, "A"),
    (BTN_B, "B"),
    (BTN_SELECT, "Select"),
    (BTN_START, "Start"),
    (BTN_UP, "Haut"),
    (BTN_DOWN, "Bas"),
    (BTN_LEFT, "Gauche"),
    (BTN_RIGHT, "Droite"),
];

/// Seuil d'un axe brut pour valoir "appuye".
pub const SEUIL: f32 = 0.5;

/// Fichier de configuration (dossier courant, a cote de `Cargo.toml` avec `cargo run`).
pub const CONFIG_FILE: &str = "puce8-manettes.txt";

/// Source physique brute d'un bouton NES (code gilrs `Code::into_u32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawSource {
    /// Bouton brut.
    Button(u32),
    /// Axe brut en dessous de -SEUIL.
    AxisMinus(u32),
    /// Axe brut au-dessus de +SEUIL.
    AxisPlus(u32),
}

impl RawSource {
    /// Source active ? `pressed(code)` / `value(code)` : etat brut de la manette.
    pub fn active(&self, pressed: &impl Fn(u32) -> bool, value: &impl Fn(u32) -> f32) -> bool {
        match *self {
            RawSource::Button(c) => pressed(c),
            RawSource::AxisMinus(c) => value(c) <= -SEUIL,
            RawSource::AxisPlus(c) => value(c) >= SEUIL,
        }
    }

    /// Texte du fichier : `b12`, `a3-`, `a3+`.
    pub fn to_text(&self) -> String {
        match *self {
            RawSource::Button(c) => format!("b{c}"),
            RawSource::AxisMinus(c) => format!("a{c}-"),
            RawSource::AxisPlus(c) => format!("a{c}+"),
        }
    }

    /// Inverse de `to_text`.
    pub fn parse(s: &str) -> Option<RawSource> {
        let s = s.trim();
        if let Some(n) = s.strip_prefix('b') {
            return n.parse().ok().map(RawSource::Button);
        }
        let n = s.strip_prefix('a')?;
        if let Some(c) = n.strip_suffix('-') {
            return c.parse().ok().map(RawSource::AxisMinus);
        }
        n.strip_suffix('+')?.parse().ok().map(RawSource::AxisPlus)
    }
}

/// Source detectee par l'assistant pour un mouvement d'axe (None si sous le seuil).
pub fn from_axis(code: u32, v: f32) -> Option<RawSource> {
    if v <= -SEUIL {
        Some(RawSource::AxisMinus(code))
    } else if v >= SEUIL {
        Some(RawSource::AxisPlus(code))
    } else {
        None
    }
}

/// Configuration d'une manette : une source (ou rien) pour chacun des 8 boutons NES.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PadConfig {
    pub sources: [Option<RawSource>; 8],
}

impl PadConfig {
    /// Boutons NES a partir de l'etat brut de la manette.
    pub fn buttons(&self, pressed: impl Fn(u32) -> bool, value: impl Fn(u32) -> f32) -> u8 {
        self.sources
            .iter()
            .zip(NES_BUTTONS)
            .filter(|(s, _)| s.is_some_and(|s| s.active(&pressed, &value)))
            .fold(0, |acc, (_, (bit, _))| acc | bit)
    }
}

/// Toutes les manettes configurees : (cle = identifiant de la manette, nom lisible, configuration).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigSet {
    pub pads: Vec<(String, String, PadConfig)>,
}

impl ConfigSet {
    /// Configuration d'une manette (par sa cle).
    pub fn get(&self, key: &str) -> Option<&PadConfig> {
        self.pads
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, _, c)| c)
    }

    /// Ajoute ou remplace la configuration d'une manette.
    pub fn set(&mut self, key: &str, name: &str, cfg: PadConfig) {
        self.pads.retain(|(k, _, _)| k != key);
        self.pads.push((key.to_string(), name.to_string(), cfg));
    }

    /// Texte du fichier : une section `[cle] nom` par manette, puis `A = b0`, `Haut = a1-`...
    pub fn to_text(&self) -> String {
        let mut t = String::from("# Puce8 : configuration des manettes (assistant F4)\n");
        for (key, name, cfg) in &self.pads {
            t.push_str(&format!("[{key}] {name}\n"));
            for (s, (_, nom)) in cfg.sources.iter().zip(NES_BUTTONS) {
                if let Some(s) = s {
                    t.push_str(&format!("{nom} = {}\n", s.to_text()));
                }
            }
        }
        t
    }

    /// Lit le texte du fichier (lignes inconnues ignorees).
    pub fn parse(text: &str) -> ConfigSet {
        let mut set = ConfigSet::default();
        for line in text.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix('[') {
                if let Some((key, name)) = rest.split_once(']') {
                    set.pads.push((
                        key.to_string(),
                        name.trim().to_string(),
                        PadConfig::default(),
                    ));
                }
            } else if let Some((nom, src)) = line.split_once('=') {
                let i = NES_BUTTONS.iter().position(|(_, n)| *n == nom.trim());
                if let (Some(i), Some(s), Some(last)) =
                    (i, RawSource::parse(src), set.pads.last_mut())
                {
                    last.2.sources[i] = Some(s);
                }
            }
        }
        set
    }

    /// Charge le fichier (vide s'il n'existe pas).
    pub fn load(path: &str) -> ConfigSet {
        std::fs::read_to_string(path)
            .map(|t| ConfigSet::parse(&t))
            .unwrap_or_default()
    }

    /// Ecrit le fichier.
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }
}

/// Cle d'une manette : son identifiant (uuid) en hexadecimal, ou son nom si l'uuid est nul.
pub fn pad_key(uuid: [u8; 16], name: &str) -> String {
    if uuid.iter().all(|&b| b == 0) {
        name.to_string()
    } else {
        uuid.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manette type SNES USB : croix = axes bruts 0 (X) et 1 (Y), A = bouton 1, B = bouton 2.
    fn snes() -> PadConfig {
        let mut c = PadConfig::default();
        c.sources[0] = Some(RawSource::Button(1));
        c.sources[1] = Some(RawSource::Button(2));
        c.sources[4] = Some(RawSource::AxisMinus(65537)); // Haut
        c.sources[5] = Some(RawSource::AxisPlus(65537)); // Bas
        c.sources[6] = Some(RawSource::AxisMinus(65536)); // Gauche
        c.sources[7] = Some(RawSource::AxisPlus(65536)); // Droite
        c
    }

    #[test]
    fn config_boutons_et_axes() {
        let c = snes();
        let rien = c.buttons(|_| false, |_| 0.0);
        assert_eq!(rien, 0);
        let b = c.buttons(
            |code| code == 1,
            |code| if code == 65537 { 1.0 } else { 0.0 },
        );
        assert_eq!(b, BTN_A | BTN_DOWN);
        let b = c.buttons(|_| false, |_| -1.0);
        assert_eq!(b, BTN_UP | BTN_LEFT);
    }

    #[test]
    fn croix_en_boutons_bruts() {
        // Manette dont la croix arrive comme des boutons (lus East/West/North par gilrs).
        let mut c = PadConfig::default();
        c.sources[5] = Some(RawSource::Button(0)); // Bas
        c.sources[6] = Some(RawSource::Button(4)); // Gauche
        c.sources[4] = Some(RawSource::Button(3)); // Haut
        assert_eq!(c.buttons(|code| code == 4, |_| 0.0), BTN_LEFT);
        assert_eq!(
            c.buttons(|code| code == 0 || code == 3, |_| 0.0),
            BTN_UP | BTN_DOWN
        );
    }

    #[test]
    fn source_texte() {
        for s in [
            RawSource::Button(12),
            RawSource::AxisMinus(65537),
            RawSource::AxisPlus(3),
        ] {
            assert_eq!(RawSource::parse(&s.to_text()), Some(s));
        }
        assert_eq!(RawSource::parse("x9"), None);
        assert_eq!(from_axis(5, -0.9), Some(RawSource::AxisMinus(5)));
        assert_eq!(from_axis(5, 0.2), None);
    }

    #[test]
    fn fichier_aller_retour() {
        let mut set = ConfigSet::default();
        set.set("0300abcd", "USB Gamepad", snes());
        set.set("autre", "Pad 2", PadConfig::default());
        let relu = ConfigSet::parse(&set.to_text());
        assert_eq!(relu, set);
        assert_eq!(relu.get("0300abcd"), Some(&snes()));
        set.set("0300abcd", "USB Gamepad", PadConfig::default()); // remplace
        assert_eq!(set.pads.len(), 2);
    }

    #[test]
    fn cle_manette() {
        assert_eq!(pad_key([0; 16], "Pad"), "Pad");
        let mut u = [0u8; 16];
        u[0] = 0x03;
        u[15] = 0xAB;
        assert_eq!(pad_key(u, "Pad"), "030000000000000000000000000000ab");
    }
}
