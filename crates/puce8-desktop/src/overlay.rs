//! Surimpressions des vues de debogage (E23b1) : fonctions PURES sur un tampon RGBA `u32`
//! (0x00RRGGBB, comme `to_rgba`).

/// Melange par composante : (src * a + dst * (255 - a)) / 255.
pub fn blend(dst: u32, src: u32, alpha: u8) -> u32 {
    let a = u32::from(alpha);
    let mut out = 0;
    for shift in [16, 8, 0] {
        let d = (dst >> shift) & 0xFF;
        let s = (src >> shift) & 0xFF;
        out |= ((s * a + d * (255 - a)) / 255) << shift;
    }
    out
}

/// Grille : colore (avec alpha) chaque pixel ou `x % step == 0 || y % step == 0`.
pub fn draw_grid(buf: &mut [u32], w: usize, h: usize, step: usize, color: u32, alpha: u8) {
    for y in 0..h {
        for x in 0..w {
            if x % step == 0 || y % step == 0 {
                let p = &mut buf[y * w + x];
                *p = blend(*p, color, alpha);
            }
        }
    }
}

/// Contour de 1 pixel d'un rectangle `pos` + `taille`, replie modulo `dim` = (w, h) (cadre de scroll).
/// Arguments groupes par paires pour rester sous la limite de clippy (pas plus de 7 arguments).
pub fn draw_rect_wrap(
    buf: &mut [u32],
    dim: (usize, usize),
    pos: (usize, usize),
    taille: (usize, usize),
    color: u32,
) {
    let ((w, h), (x, y), (rw, rh)) = (dim, pos, taille);
    for dx in 0..rw {
        let px = (x + dx) % w;
        buf[(y % h) * w + px] = color;
        buf[((y + rh - 1) % h) * w + px] = color;
    }
    for dy in 0..rh {
        let py = (y + dy) % h;
        buf[py * w + (x % w)] = color;
        buf[py * w + ((x + rw - 1) % w)] = color;
    }
}

/// Pixel de l'image sous la souris : `pos` et `origin` en points ecran, `zoom` = taille d'un pixel.
/// `None` si la souris est hors de l'image (w x h).
pub fn pixel_from_hover(
    pos: (f32, f32),
    origin: (f32, f32),
    zoom: f32,
    w: usize,
    h: usize,
) -> Option<(usize, usize)> {
    let fx = (pos.0 - origin.0) / zoom;
    let fy = (pos.1 - origin.1) / zoom;
    if fx < 0.0 || fy < 0.0 {
        return None;
    }
    let (x, y) = (fx as usize, fy as usize);
    if x < w && y < h {
        Some((x, y))
    } else {
        None
    }
}

/// Tampon `u32` -> octets RGBA pour une texture egui (alpha = 255).
pub fn to_bytes(buf: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() * 4);
    for &c in buf {
        out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 0xFF]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_moitie() {
        let c = blend(0x000000, 0xFFFFFF, 128);
        for shift in [16, 8, 0] {
            let v = (c >> shift) & 0xFF;
            assert!(v == 127 || v == 128, "composante {v}");
        }
        assert_eq!(blend(0x123456, 0xABCDEF, 255), 0xABCDEF);
        assert_eq!(blend(0x123456, 0xABCDEF, 0), 0x123456);
    }

    #[test]
    fn grille_pas_8() {
        let mut buf = vec![0u32; 16 * 16];
        draw_grid(&mut buf, 16, 16, 8, 0xFFFFFF, 255);
        assert_eq!(buf[3 * 16 + 8], 0xFFFFFF); // (8, 3) colore
        assert_eq!(buf[3 * 16 + 7], 0); // (7, 3) non
        assert_eq!(buf[8 * 16 + 5], 0xFFFFFF); // (5, 8) colore (ligne y = 8)
    }

    #[test]
    fn rect_wrap() {
        let (w, h) = (512, 480);
        let mut buf = vec![0u32; w * h];
        draw_rect_wrap(&mut buf, (w, h), (500, 10), (256, 240), 0xFFFF00);
        assert_eq!(buf[20 * w + 500], 0xFFFF00); // bord gauche en x = 500
        assert_eq!(buf[20 * w + 243], 0xFFFF00); // bord droit en (500 + 255) % 512 = 243
        assert_eq!(buf[20 * w + 100], 0); // interieur vide
    }

    #[test]
    fn rect_wrap_vertical() {
        let (w, h) = (512, 480);
        let mut buf = vec![0u32; w * h];
        draw_rect_wrap(&mut buf, (w, h), (0, 400), (256, 240), 0xFFFF00);
        assert_eq!(buf[400 * w + 10], 0xFFFF00); // haut en y = 400
        assert_eq!(buf[159 * w + 10], 0xFFFF00); // bas en (400 + 239) % 480 = 159
    }

    #[test]
    fn pixel_depuis_survol() {
        assert_eq!(
            pixel_from_hover((13.0, 7.0), (10.0, 4.0), 3.0, 256, 128),
            Some((1, 1))
        );
        assert_eq!(
            pixel_from_hover((5.0, 7.0), (10.0, 4.0), 3.0, 256, 128),
            None
        );
        assert_eq!(
            pixel_from_hover((10.0 + 256.0 * 3.0, 7.0), (10.0, 4.0), 3.0, 256, 128),
            None
        );
    }

    #[test]
    fn octets_rgba() {
        assert_eq!(to_bytes(&[0x123456]), vec![0x12, 0x34, 0x56, 0xFF]);
    }
}
