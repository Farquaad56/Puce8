//! Entrees joueur (E24b1) : logique PURE (sans gilrs), testable.
//! Clavier egui et manettes gilrs sont branches en E24b2.
//! Correctif E24c (croix bas/gauche/haut sans effet) : etat suivi par evenements, croix en axes,
//! croix prioritaire sur le stick.

use eframe::egui::Key;
use puce8_core::controller::{
    BTN_A, BTN_B, BTN_DOWN, BTN_LEFT, BTN_RIGHT, BTN_SELECT, BTN_START, BTN_UP,
};

/// Seuil du stick analogique pour une direction.
pub const STICK_SEUIL: f32 = 0.5;

/// Boutons d'une manette physique, independants de gilrs (traduits en E24b2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadButton {
    East,
    South,
    West,
    Select,
    Start,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

/// Nombre de variantes de `PadButton`.
pub const NB_PAD_BUTTONS: usize = 9;

/// Axes d'une manette physique utiles a la NES (convention gilrs : Y positif = haut).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadAxis {
    StickX,
    StickY,
    /// Croix vue comme un axe (chapeau / "hat" des manettes USB generiques).
    DPadX,
    DPadY,
}

/// Valeurs des axes (0.0 si absents).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PadAxes {
    pub stick_x: f32,
    pub stick_y: f32,
    pub dpad_x: f32,
    pub dpad_y: f32,
}

impl PadAxes {
    /// Valeur d'un axe.
    pub fn get(&self, a: PadAxis) -> f32 {
        match a {
            PadAxis::StickX => self.stick_x,
            PadAxis::StickY => self.stick_y,
            PadAxis::DPadX => self.dpad_x,
            PadAxis::DPadY => self.dpad_y,
        }
    }

    /// Modifie un axe.
    pub fn set(&mut self, a: PadAxis, v: f32) {
        match a {
            PadAxis::StickX => self.stick_x = v,
            PadAxis::StickY => self.stick_y = v,
            PadAxis::DPadX => self.dpad_x = v,
            PadAxis::DPadY => self.dpad_y = v,
        }
    }
}

/// Etat d'une manette reconstruit a partir des EVENEMENTS gilrs (E24c) : ne depend pas de la
/// table de correspondance de gilrs, qui peut ne pas lier les directions de la croix de
/// certaines manettes (seule la droite repondait).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PadTrack {
    pressed: [bool; NB_PAD_BUTTONS],
    pub axes: PadAxes,
}

impl PadTrack {
    /// Bouton appuye (`true`) ou relache (`false`).
    pub fn set_button(&mut self, b: PadButton, down: bool) {
        self.pressed[b as usize] = down;
    }

    /// Bouton appuye d'apres les evenements recus.
    pub fn is_pressed(&self, b: PadButton) -> bool {
        self.pressed[b as usize]
    }
}

/// Garde la valeur la plus eloignee de 0 (deux sources pour le meme axe).
pub fn plus_fort(a: f32, b: f32) -> f32 {
    if a.abs() >= b.abs() {
        a
    } else {
        b
    }
}

/// Clavier (joueur 1) : W = A, X = B, Backspace = Select, Entree = Start, fleches = croix.
pub fn keyboard_buttons(down: impl Fn(Key) -> bool) -> u8 {
    let table = [
        (Key::W, BTN_A),
        (Key::X, BTN_B),
        (Key::Backspace, BTN_SELECT),
        (Key::Enter, BTN_START),
        (Key::ArrowUp, BTN_UP),
        (Key::ArrowDown, BTN_DOWN),
        (Key::ArrowLeft, BTN_LEFT),
        (Key::ArrowRight, BTN_RIGHT),
    ];
    table
        .iter()
        .filter(|(k, _)| down(*k))
        .fold(0, |acc, (_, b)| acc | b)
}

/// Bits de direction NES.
fn directions(haut: bool, bas: bool, gauche: bool, droite: bool) -> u8 {
    let mut b = 0;
    if haut {
        b |= BTN_UP;
    }
    if bas {
        b |= BTN_DOWN;
    }
    if gauche {
        b |= BTN_LEFT;
    }
    if droite {
        b |= BTN_RIGHT;
    }
    b
}

/// Manette : A = East (position du A de la NES), B = South ou West, Select, Start.
/// Croix = boutons DPad OU axes DPadX/DPadY (seuil 0,5 ; Y positif = haut).
/// Stick gauche : utilise SEULEMENT si la croix est relachee (un stick qui derive ou mal
/// calibre ne doit pas annuler une direction de la croix via `sanitize`).
pub fn pad_buttons(pressed: impl Fn(PadButton) -> bool, axes: PadAxes) -> u8 {
    let mut b = 0;
    if pressed(PadButton::East) {
        b |= BTN_A;
    }
    if pressed(PadButton::South) || pressed(PadButton::West) {
        b |= BTN_B;
    }
    if pressed(PadButton::Select) {
        b |= BTN_SELECT;
    }
    if pressed(PadButton::Start) {
        b |= BTN_START;
    }
    let croix = directions(
        pressed(PadButton::DPadUp) || axes.dpad_y > STICK_SEUIL,
        pressed(PadButton::DPadDown) || axes.dpad_y < -STICK_SEUIL,
        pressed(PadButton::DPadLeft) || axes.dpad_x < -STICK_SEUIL,
        pressed(PadButton::DPadRight) || axes.dpad_x > STICK_SEUIL,
    );
    let dirs = if croix != 0 {
        croix
    } else {
        directions(
            axes.stick_y > STICK_SEUIL,
            axes.stick_y < -STICK_SEUIL,
            axes.stick_x < -STICK_SEUIL,
            axes.stick_x > STICK_SEUIL,
        )
    };
    b | dirs
}

/// Clavier et manette du meme joueur : OU logique.
pub fn merge(clavier: u8, manette: u8) -> u8 {
    clavier | manette
}

/// Supprime les directions opposees (Haut + Bas, Gauche + Droite) : certains jeux plantent sinon.
pub fn sanitize(b: u8) -> u8 {
    let mut out = b;
    if b & (BTN_UP | BTN_DOWN) == BTN_UP | BTN_DOWN {
        out &= !(BTN_UP | BTN_DOWN);
    }
    if b & (BTN_LEFT | BTN_RIGHT) == BTN_LEFT | BTN_RIGHT {
        out &= !(BTN_LEFT | BTN_RIGHT);
    }
    out
}

/// Boutons NES en texte (fenetre de diagnostic des manettes, E24c).
pub fn buttons_text(b: u8) -> String {
    let noms = [
        (BTN_A, "A"),
        (BTN_B, "B"),
        (BTN_SELECT, "Select"),
        (BTN_START, "Start"),
        (BTN_UP, "Haut"),
        (BTN_DOWN, "Bas"),
        (BTN_LEFT, "Gauche"),
        (BTN_RIGHT, "Droite"),
    ];
    let v: Vec<&str> = noms
        .iter()
        .filter(|(bit, _)| b & bit != 0)
        .map(|(_, n)| *n)
        .collect();
    if v.is_empty() {
        "-".to_string()
    } else {
        v.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Axes au repos.
    const REPOS: PadAxes = PadAxes {
        stick_x: 0.0,
        stick_y: 0.0,
        dpad_x: 0.0,
        dpad_y: 0.0,
    };

    fn stick(x: f32, y: f32) -> PadAxes {
        PadAxes {
            stick_x: x,
            stick_y: y,
            ..REPOS
        }
    }

    #[test]
    fn clavier() {
        let b = keyboard_buttons(|k| k == Key::W || k == Key::ArrowRight || k == Key::Enter);
        assert_eq!(b, BTN_A | BTN_RIGHT | BTN_START);
        assert_eq!(keyboard_buttons(|_| false), 0);
    }

    #[test]
    fn manette_boutons_et_stick() {
        let b = pad_buttons(|p| p == PadButton::East || p == PadButton::West, REPOS);
        assert_eq!(b, BTN_A | BTN_B);
        assert_eq!(pad_buttons(|_| false, stick(0.8, 0.9)), BTN_RIGHT | BTN_UP);
        assert_eq!(
            pad_buttons(|_| false, stick(-0.8, -0.9)),
            BTN_LEFT | BTN_DOWN
        );
        assert_eq!(pad_buttons(|_| false, stick(0.4, -0.4)), 0); // sous le seuil
    }

    // ---------- E24c : correctif croix ----------

    #[test]
    fn croix_4_directions() {
        let une = |d: PadButton| pad_buttons(move |p| p == d, REPOS);
        assert_eq!(une(PadButton::DPadUp), BTN_UP);
        assert_eq!(une(PadButton::DPadDown), BTN_DOWN);
        assert_eq!(une(PadButton::DPadLeft), BTN_LEFT);
        assert_eq!(une(PadButton::DPadRight), BTN_RIGHT);
    }

    #[test]
    fn croix_prioritaire_sur_stick() {
        // Stick qui derive vers la droite et le haut : la croix doit quand meme passer.
        let derive = stick(0.9, 0.9);
        let b = pad_buttons(|p| p == PadButton::DPadLeft, derive);
        assert_eq!(sanitize(b), BTN_LEFT);
        let b = pad_buttons(|p| p == PadButton::DPadDown, derive);
        assert_eq!(sanitize(b), BTN_DOWN);
        // Croix relachee : le stick reprend la main.
        assert_eq!(pad_buttons(|_| false, derive), BTN_RIGHT | BTN_UP);
    }

    #[test]
    fn croix_en_axes() {
        let hat = |x: f32, y: f32| {
            pad_buttons(
                |_| false,
                PadAxes {
                    dpad_x: x,
                    dpad_y: y,
                    ..REPOS
                },
            )
        };
        assert_eq!(hat(0.0, 1.0), BTN_UP);
        assert_eq!(hat(0.0, -1.0), BTN_DOWN);
        assert_eq!(hat(-1.0, 0.0), BTN_LEFT);
        assert_eq!(hat(1.0, 0.0), BTN_RIGHT);
        assert_eq!(hat(-1.0, -1.0), BTN_LEFT | BTN_DOWN); // diagonale
    }

    #[test]
    fn suivi_par_evenements() {
        let mut t = PadTrack::default();
        t.set_button(PadButton::DPadLeft, true);
        t.axes.set(PadAxis::DPadY, -1.0);
        assert!(t.is_pressed(PadButton::DPadLeft));
        assert!(!t.is_pressed(PadButton::DPadRight));
        assert_eq!(t.axes.get(PadAxis::DPadY), -1.0);
        let b = pad_buttons(|p| t.is_pressed(p), t.axes);
        assert_eq!(b, BTN_LEFT | BTN_DOWN);
        t.set_button(PadButton::DPadLeft, false);
        t.axes.set(PadAxis::DPadY, 0.0);
        assert_eq!(pad_buttons(|p| t.is_pressed(p), t.axes), 0);
    }

    #[test]
    fn deux_sources() {
        assert_eq!(plus_fort(0.0, -1.0), -1.0);
        assert_eq!(plus_fort(0.7, -0.2), 0.7);
    }

    #[test]
    fn texte() {
        assert_eq!(buttons_text(0), "-");
        assert_eq!(buttons_text(BTN_A | BTN_LEFT), "A Gauche");
    }

    #[test]
    fn merge_ou() {
        assert_eq!(merge(BTN_A, BTN_LEFT), BTN_A | BTN_LEFT);
        assert_eq!(merge(BTN_A, BTN_A), BTN_A);
    }

    #[test]
    fn sanitize_haut_bas() {
        assert_eq!(sanitize(BTN_UP | BTN_DOWN | BTN_A), BTN_A);
        assert_eq!(sanitize(BTN_LEFT | BTN_RIGHT | BTN_UP), BTN_UP);
        assert_eq!(sanitize(BTN_UP | BTN_RIGHT), BTN_UP | BTN_RIGHT);
    }
}
