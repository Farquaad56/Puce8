use puce8_core::ppu::palette::to_rgba;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

/// Encode le framebuffer (indices palette u16) en PNG RGBA 256x240, en memoire.
pub fn encode_png(fb: &[u16]) -> Result<Vec<u8>, String> {
    if fb.len() != WIDTH * HEIGHT {
        return Err(format!(
            "framebuffer de taille {} (attendu {})",
            fb.len(),
            WIDTH * HEIGHT
        ));
    }
    let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
    for (i, &px) in fb.iter().enumerate() {
        let v = to_rgba(px);
        rgba[i * 4] = ((v >> 16) & 0xFF) as u8;
        rgba[i * 4 + 1] = ((v >> 8) & 0xFF) as u8;
        rgba[i * 4 + 2] = (v & 0xFF) as u8;
        rgba[i * 4 + 3] = 0xFF;
    }

    let mut out: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Ecrit une capture PNG dans `path` (le dossier doit exister).
pub fn write_screenshot(path: &str, fb: &[u16]) -> Result<(), String> {
    let bytes = encode_png(fb)?;
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {}", path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip_256x240() {
        let fb: Vec<u16> = (0..WIDTH * HEIGHT).map(|i| i as u16 % 96).collect();
        let png_bytes = encode_png(&fb).expect("encode");

        let decoder = png::Decoder::new(Cursor::new(&png_bytes));
        let mut reader = decoder.read_info().expect("decode header");
        assert_eq!(reader.info().width as usize, WIDTH);
        assert_eq!(reader.info().height as usize, HEIGHT);

        let mut out = vec![0u8; reader.output_buffer_size()];
        reader.next_frame(&mut out).expect("decode frame");

        for (i, &px) in fb.iter().enumerate() {
            let v = to_rgba(px);
            assert_eq!(out[i * 4], ((v >> 16) & 0xFF) as u8, "R pixel {}", i);
            assert_eq!(out[i * 4 + 1], ((v >> 8) & 0xFF) as u8, "G pixel {}", i);
            assert_eq!(out[i * 4 + 2], (v & 0xFF) as u8, "B pixel {}", i);
            assert_eq!(out[i * 4 + 3], 0xFF, "A pixel {}", i);
        }
    }

    #[test]
    fn taille_invalide() {
        let err = encode_png(&[0; WIDTH * HEIGHT - 1]).unwrap_err();
        assert!(err.contains("taille"));
    }
}
