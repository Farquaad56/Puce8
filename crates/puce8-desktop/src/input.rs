//! Entrees joueur (E24b1) : logique PURE (sans gilrs), testable.
//! Clavier egui et manettes gilrs sont branches en E24b2.

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

/// Manette : A = East (position du A de la NES), B = South ou West, Select, Start,
/// croix = DPad OU stick gauche (seuil 0,5 ; Y positif = haut).
pub fn pad_buttons(pressed: impl Fn(PadButton) -> bool, stick_x: f32, stick_y: f32) -> u8 {
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
    if pressed(PadButton::DPadUp) || stick_y > STICK_SEUIL {
        b |= BTN_UP;
    }
    if pressed(PadButton::DPadDown) || stick_y < -STICK_SEUIL {
        b |= BTN_DOWN;
    }
    if pressed(PadButton::DPadLeft) || stick_x < -STICK_SEUIL {
        b |= BTN_LEFT;
    }
    if pressed(PadButton::DPadRight) || stick_x > STICK_SEUIL {
        b |= BTN_RIGHT;
    }
    b
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clavier() {
        let b = keyboard_buttons(|k| k == Key::W || k == Key::ArrowRight || k == Key::Enter);
        assert_eq!(b, BTN_A | BTN_RIGHT | BTN_START);
        assert_eq!(keyboard_buttons(|_| false), 0);
    }

    #[test]
    fn manette_boutons_et_stick() {
        let b = pad_buttons(|p| p == PadButton::East || p == PadButton::West, 0.0, 0.0);
        assert_eq!(b, BTN_A | BTN_B);
        assert_eq!(pad_buttons(|_| false, 0.8, 0.9), BTN_RIGHT | BTN_UP);
        assert_eq!(pad_buttons(|_| false, -0.8, -0.9), BTN_LEFT | BTN_DOWN);
        assert_eq!(pad_buttons(|_| false, 0.4, -0.4), 0); // sous le seuil
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
