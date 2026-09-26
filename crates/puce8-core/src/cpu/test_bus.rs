//! Bus de test : memoire 64 Ko + journal des acces (1 entree = 1 cycle).

use super::CpuBus;

/// Acces journalise par le [TestBus].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read(u16, u8),
    Write(u16, u8),
}

/// Callback avant un acces : index du cycle + lignes NMI/IRQ (modifiables).
pub type AccessHook = dyn FnMut(usize, &mut bool, &mut bool);

/// Bus de test : memoire 64 Ko + journal des acces (1 entree = 1 cycle).
pub struct TestBus {
    pub mem: Box<[u8; 0x10000]>,
    /// 1 entree par acces = 1 par cycle.
    pub log: Vec<Access>,
    pub nmi: bool,
    pub irq: bool,
    /// appele AVANT chaque acces avec l'index du cycle (log.len())
    pub on_access: Option<Box<AccessHook>>,
}

impl TestBus {
    pub fn new() -> Self {
        TestBus {
            mem: Box::new([0u8; 0x10000]),
            log: Vec::new(),
            nmi: false,
            irq: false,
            on_access: None,
        }
    }

    /// Charge `bytes` en memoire a partir de `addr` (repli au-dela de 64 Ko).
    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            let idx = (usize::from(addr) + i) % 0x10000;
            self.mem[idx] = b;
        }
    }

    /// Vecteur d'interruption little-endian : octet bas a `vector`, haut a `vector + 1`.
    pub fn set_vector(&mut self, vector: u16, target: u16) {
        let lo = usize::from(vector);
        let hi = usize::from(vector.wrapping_add(1));
        self.mem[lo] = (target & 0xFF) as u8;
        self.mem[hi] = (target >> 8) as u8;
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }

    /// Callback avant chaque acces, avec l'index du cycle (log.len()).
    fn before_access(&mut self) {
        if let Some(cb) = &mut self.on_access {
            cb(self.log.len(), &mut self.nmi, &mut self.irq);
        }
    }
}

impl CpuBus for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.before_access();
        let value = self.mem[usize::from(addr)];
        self.log.push(Access::Read(addr, value));
        value
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.before_access();
        self.mem[usize::from(addr)] = value;
        self.log.push(Access::Write(addr, value));
    }

    fn peek(&self, addr: u16) -> u8 {
        self.mem[usize::from(addr)]
    }

    fn nmi_line(&self) -> bool {
        self.nmi
    }

    fn irq_line(&self) -> bool {
        self.irq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testbus_journal() {
        let mut bus = TestBus::new();
        bus.load(0x100, &[0xAB]); // la charge n'est pas journalisee
        assert_eq!(bus.read(0x10), 0);
        bus.write(0x20, 1);
        assert_eq!(bus.log, vec![Access::Read(0x10, 0), Access::Write(0x20, 1)]);
        let _ = bus.peek(0x30); // sans aucun effet -> pas journalise
        assert_eq!(bus.log.len(), 2);
        bus.clear_log();
        assert!(bus.log.is_empty());
    }

    #[test]
    fn testbus_vecteur() {
        let mut bus = TestBus::new();
        bus.set_vector(0xFFFC, 0x8000);
        assert_eq!(bus.mem[0xFFFC], 0x00);
        assert_eq!(bus.mem[0xFFFD], 0x80);
    }

    #[test]
    fn testbus_on_access() {
        let mut bus = TestBus::new();
        bus.on_access = Some(Box::new(|idx, nmi, _irq| {
            if idx == 2 {
                *nmi = true;
            }
        }));
        assert!(!bus.nmi_line());
        bus.read(0x10); // index 0
        bus.write(0x20, 1); // index 1
        assert!(!bus.nmi_line()); // faux apres 2 acces
        bus.read(0x30); // index 2 -> le callback pose nmi
        assert!(bus.nmi_line()); // vrai apres 3 acces
    }
}
