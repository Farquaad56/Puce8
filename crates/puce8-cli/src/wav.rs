//! Fichiers WAV (E34b1) : PCM 16 bits mono, en-tete RIFF de 44 octets ; RMS par fenetre (E34b2).

/// Taux d'echantillonnage des fichiers ecrits (Hz).
pub const RATE: u32 = 44_100;

/// En-tete RIFF/WAVE de 44 octets pour `n` echantillons 16 bits mono a `rate` Hz, puis les donnees.
pub fn wav_bytes(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + samples.len() * 2);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes()); // taille du bloc fmt
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes()); // octets par seconde
    v.extend_from_slice(&2u16.to_le_bytes()); // octets par echantillon
    v.extend_from_slice(&16u16.to_le_bytes()); // bits par echantillon
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let x = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        v.extend_from_slice(&x.to_le_bytes());
    }
    v
}

/// Ecrit un fichier WAV.
pub fn write_wav(path: &str, samples: &[f32], rate: u32) -> std::io::Result<()> {
    std::fs::write(path, wav_bytes(samples, rate))
}

fn tag(bytes: &[u8], at: usize, t: &[u8]) -> bool {
    bytes.get(at..at + t.len()) == Some(t)
}

/// Relit un WAV ecrit par `wav_bytes` : (taux, echantillons dans [-1, 1]) ; None si autre format.
pub fn read_wav(bytes: &[u8]) -> Option<(u32, Vec<f32>)> {
    if bytes.len() < 44 || !tag(bytes, 0, b"RIFF") || !tag(bytes, 8, b"WAVE") {
        return None;
    }
    if !tag(bytes, 12, b"fmt ") || !tag(bytes, 36, b"data") {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    if u16_at(20) != 1 || u16_at(22) != 1 || u16_at(34) != 16 {
        return None;
    }
    let rate = u32_at(24);
    let len = (u32_at(40) as usize).min(bytes.len() - 44);
    let samples = bytes[44..44 + len]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0)
        .collect();
    Some((rate, samples))
}

// ---------- E34b2 : statistiques ----------

/// RMS de chaque fenetre de `window_ms` millisecondes.
pub fn rms_windows(samples: &[f32], rate: u32, window_ms: u32) -> Vec<f32> {
    let n = (rate as usize * window_ms as usize / 1000).max(1);
    samples
        .chunks(n)
        .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
        .collect()
}

/// Resume de `wav-stats`.
#[derive(Debug, Clone, PartialEq)]
pub struct Resume {
    /// Nombre de zones fortes (fenetres consecutives avec RMS >= 50 % du maximum).
    pub zones: usize,
    /// RMS maximal.
    pub fort_max: f32,
    /// RMS des fenetres faibles situees entre la 1re et la derniere zone forte.
    pub entre_min: f32,
    pub entre_mediane: f32,
    pub entre_max: f32,
}

/// Zones fortes et energie entre elles.
pub fn resume(rms: &[f32]) -> Resume {
    let fort_max = rms.iter().copied().fold(0.0f32, f32::max);
    let fort: Vec<bool> = rms
        .iter()
        .map(|&r| fort_max > 0.0 && r >= fort_max * 0.5)
        .collect();
    let zones = fort.windows(2).filter(|w| !w[0] && w[1]).count()
        + usize::from(fort.first() == Some(&true));
    let mut entre: Vec<f32> = Vec::new();
    if let (Some(a), Some(b)) = (fort.iter().position(|&f| f), fort.iter().rposition(|&f| f)) {
        entre = (a..=b).filter(|&i| !fort[i]).map(|i| rms[i]).collect();
    }
    entre.sort_by(|x, y| x.total_cmp(y));
    Resume {
        zones,
        fort_max,
        entre_min: entre.first().copied().unwrap_or(0.0),
        entre_mediane: entre.get(entre.len() / 2).copied().unwrap_or(0.0),
        entre_max: entre.last().copied().unwrap_or(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_entete() {
        let b = wav_bytes(&[0.0, 1.0, -1.0], RATE);
        assert_eq!(b.len(), 44 + 6);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([b[4], b[5], b[6], b[7]]), 36 + 6);
        assert_eq!(&b[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes([b[24], b[25], b[26], b[27]]), 44_100);
        assert_eq!(&b[36..40], b"data");
        assert_eq!(u32::from_le_bytes([b[40], b[41], b[42], b[43]]), 6);
        assert_eq!(i16::from_le_bytes([b[46], b[47]]), 32767);
    }

    #[test]
    fn wav_aller_retour() {
        let s = [0.0f32, 0.5, -0.25];
        let (rate, lu) = read_wav(&wav_bytes(&s, RATE)).unwrap();
        assert_eq!(rate, RATE);
        assert_eq!(lu.len(), 3);
        assert!((lu[1] - 0.5).abs() < 0.001 && (lu[2] + 0.25).abs() < 0.001);
        assert!(read_wav(b"pas un wav").is_none());
    }

    // ---------- E34b2 ----------

    #[test]
    fn rms_et_resume() {
        // bip (carre 0,5) 0,2 s ; quasi-silence 0,4 s ; bip 0,2 s.
        let bip = |n: usize| (0..n).map(|i| if i % 50 < 25 { 0.5 } else { -0.5 });
        let mut s: Vec<f32> = bip(8820).collect();
        s.extend(std::iter::repeat_n(0.01, 17_640));
        s.extend(bip(8820));
        let rms = rms_windows(&s, RATE, 50);
        assert_eq!(rms.len(), 16); // 35 280 echantillons / 2 205 par fenetre
        assert!((rms[0] - 0.5).abs() < 0.01);
        let r = resume(&rms);
        assert_eq!(r.zones, 2);
        assert!(r.entre_mediane < 0.1 * r.fort_max);
    }
}
