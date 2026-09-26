//! Fond PPU (E18a) : increments et copies du registre v (fonctions pures).
//! Layout de v : yyy NN YYYYY XXXXX (fine Y, nametable, coarse Y, coarse X).

/// Incremente coarse X ; a 31 : repasse a 0 et bascule la nametable horizontale.
pub fn inc_coarse_x(v: u16) -> u16 {
    if (v & 0x001F) == 31 {
        (v & !0x001F) ^ 0x0400
    } else {
        v.wrapping_add(1)
    }
}

/// Incremente fine Y ; au debordement, incremente coarse Y (29 -> 0 + bascule NT verticale ; 31 -> 0 sans bascule).
pub fn inc_y(v: u16) -> u16 {
    if (v & 0x7000) != 0x7000 {
        return v.wrapping_add(0x1000);
    }
    let mut v = v & !0x7000;
    let mut y = (v >> 5) & 31;
    if y == 29 {
        y = 0;
        v ^= 0x0800;
    } else if y == 31 {
        y = 0;
    } else {
        y += 1;
    }
    (v & !0x03E0) | (y << 5)
}

/// Copie les bits horizontaux de t dans v (coarse X + nametable X).
pub fn copy_x(v: u16, t: u16) -> u16 {
    (v & !0x041F) | (t & 0x041F)
}

/// Copie les bits verticaux de t dans v (fine Y + nametable Y + coarse Y).
pub fn copy_y(v: u16, t: u16) -> u16 {
    (v & !0x7BE0) | (t & 0x7BE0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inc_x_simple() {
        assert_eq!(inc_coarse_x(0x0000), 0x0001);
        assert_eq!(inc_coarse_x(0x0005), 0x0006);
    }

    #[test]
    fn inc_x_31_change_nt() {
        assert_eq!(inc_coarse_x(0x001F), 0x0400);
        assert_eq!(inc_coarse_x(0x041F), 0x0000);
    }

    #[test]
    fn inc_y_fin() {
        assert_eq!(inc_y(0x1000), 0x2000);
    }

    #[test]
    fn inc_y_29() {
        let v = 0x7000 | (29 << 5); // fine Y = 7, coarse Y = 29
        assert_eq!(inc_y(v), 0x0800);
        assert_eq!(inc_y(v | 0x0800), 0x0000);
    }

    #[test]
    fn inc_y_31() {
        let v = 0x7000 | (31 << 5); // fine Y = 7, coarse Y = 31
        assert_eq!(inc_y(v), 0x0000);
    }

    #[test]
    fn copy_x_bits() {
        assert_eq!(copy_x(0x7BE0, 0x041F), 0x7FFF);
        assert_eq!(copy_x(0x7FFF, 0x0000), 0x7BE0);
    }

    #[test]
    fn copy_y_bits() {
        assert_eq!(copy_y(0x041F, 0x7BE0), 0x7FFF);
        assert_eq!(copy_y(0x7FFF, 0x0000), 0x041F);
    }
}
