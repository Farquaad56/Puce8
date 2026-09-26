//! Bus CPU : contrat vu par le processeur (E03) + bus de test pour les tests unitaires.

/// Contrat du bus vu par le 6502. Le bus ne fait pas avancer le temps :
/// un accès = un cycle, mais c'est `Cpu::tick()` (E04), puis `Nes::tick()`, qui cadence tout.
pub trait CpuBus {
    /// Lecture avec effets de bord (open bus, $2007, ...).
    fn read(&mut self, addr: u16) -> u8;
    /// Écriture avec effets de bord.
    fn write(&mut self, addr: u16, value: u8);
    /// Lecture sans aucun effet : même valeur que `read`, ni journal ni callback.
    fn peek(&self, addr: u16) -> u8;
    /// true = NMI demandée.
    fn nmi_line(&self) -> bool;
    /// true = IRQ active (sensible au niveau).
    fn irq_line(&self) -> bool;
}

#[cfg(test)]
pub(crate) mod test_bus;
