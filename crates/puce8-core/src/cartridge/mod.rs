//! Cartouche : lecture du fichier `.nes` (en-tête iNES / NES 2.0).

pub mod ines;

pub use ines::{Cartridge, RomError};
