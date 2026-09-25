# Journal Puce8 (append-only, 1 ligne = 1 fait utile)

- E00 (2026-09-26) : workspace de 3 crates créé ; `cargo new` hérite edition/version de `[workspace.package]` ; puce8-core zéro dépendance.
- nes-test-roms cloné dans tests/roms (ignoré par git) ; nestest.log = 8991 lignes, vérifié.
- verifier.ps1 : les commandes natives ne lèvent pas d'erreur PowerShell en cas d'échec → vérifier `$LASTEXITCODE` après chaque commande.
