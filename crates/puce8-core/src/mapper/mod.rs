//! Mappers de cartouche. À l'étape E01, seul l'enum `Mirroring` existe (ARCHI §A3).

/// Organisation des nametables vue par le PPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    SingleScreenLower,
    SingleScreenUpper,
    FourScreen,
}
