//! DMA OAM ($4014) : machine a etats qui vole des cycles au CPU (E16b).

/// Parite du cycle "get" (lecture possible) : `cpu_cycles % 2 == GET_PARITY`.
pub const GET_PARITY: u64 = 0;

/// Machine a etats de la DMA OAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OamDma {
    /// Aucune DMA en cours.
    Idle,
    /// Ecriture $4014 recue : l'arret attend un acces lecture du CPU.
    Requested(u8),
    /// Cycle d'arret (aucun acces).
    // reserve E36
    Halt(u8),
    /// Cycle d'alignement (aucun acces) : le cycle suivant etait un put.
    Align(u8),
    /// Lecture RAM en attente.
    Get { page: u8, i: u8 },
    /// Ecriture OAM en attente.
    Put { page: u8, i: u8, v: u8 },
}

impl OamDma {
    /// Demande une copie OAM depuis la page `page` (la derniere ecriture gagne).
    pub fn request_oam(&mut self, page: u8) {
        *self = OamDma::Requested(page);
    }
}
