# Journal Puce8 (append-only, 1 ligne = 1 fait utile)

- E00 (2026-09-26) : workspace de 3 crates créé ; `cargo new` hérite edition/version de `[workspace.package]` ; puce8-core zéro dépendance.
- nes-test-roms cloné dans tests/roms (ignoré par git) ; nestest.log = 8991 lignes, vérifié.
- verifier.ps1 : les commandes natives ne lèvent pas d'erreur PowerShell en cas d'échec → vérifier `$LASTEXITCODE` après chaque commande.
- E01 : iNES « sale » détecté par octets 12-15 non nuls (pas seulement 7-15) → mapper = octet6 >> 4. Clippy récent refuse `repeat().take()` → `Vec::resize` dans le helper de test.
- E01 : NES 2.0 : `64 << n` avec n = 0 → taille 0 (wiki) ; PRG-RAM = max(volatile, NVRAM) si octet 10 ≠ 0, sinon 8 Ko ; CHR-RAM min 8 Ko si CHR-ROM = 0.
