//! Cartouche : lecture du fichier `.nes` (en-tete iNES / NES 2.0).

pub mod ines;

pub use ines::{Cartridge, RomError};
