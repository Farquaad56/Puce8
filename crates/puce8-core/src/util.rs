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

#[cfg(test)]
mod tests {
    use super::fnv1a64;

    #[test]
    fn fnv1a64_vide() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
    }

    #[test]
    fn fnv1a64_a() {
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }
}
