/// Hachage FNV-1a 64 bits (base d'offset `0xcbf29ce484222325`, prime `0x100000001b3`).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Hachage d'une image : FNV-1a 64 sur les octets petit-boutistes de chaque pixel.
pub fn frame_hash(fb: &[u16]) -> u64 {
    let mut bytes = Vec::with_capacity(fb.len() * 2);
    for &px in fb {
        bytes.extend_from_slice(&px.to_le_bytes());
    }
    fnv1a64(&bytes)
}

#[cfg(test)]
mod tests {
    use super::{fnv1a64, frame_hash};

    #[test]
    fn fnv1a64_vide() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
    }

    #[test]
    fn fnv1a64_a() {
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn frame_hash_petit_boutiste() {
        assert_eq!(frame_hash(&[0x0102]), fnv1a64(&[0x02, 0x01]));
        assert_eq!(frame_hash(&[]), fnv1a64(b""));
    }
}
