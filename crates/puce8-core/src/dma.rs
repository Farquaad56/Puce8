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

/// Machine a etats de la DMC DMA (E33b1) : arret, factice, alignement eventuel, get (3 ou 4 cycles).
// SIMPLIFICATION: pas de distinction entre DMA de chargement et de rechargement (revu en E36).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmcDma {
    /// Aucune DMC DMA en cours.
    Idle,
    /// Octet demande a cette adresse : l'arret attend un cycle de lecture du CPU.
    Requested(u16),
    /// Cycle factice (aucun acces).
    Dummy(u16),
    /// Cycle d'alignement (aucun acces) : le cycle suivant sera un get.
    Align(u16),
    /// Cycle get : lecture de l'octet, puis `Apu::dmc_dma_complete`.
    Get(u16),
}
