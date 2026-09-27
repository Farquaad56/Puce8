# ROM de test de Puce8

Les ROM de test **ne sont pas dans ce dépôt** (dossier `tests/` ignoré par git). Elles ne servent **pas** à compiler :
`cargo build --workspace` fonctionne sans elles. Elles servent aux tests (`cargo test`) et à la suite de ROM (`puce8-cli suite`).

## Où les mettre

Toutes les commandes se lancent depuis la racine du dépôt (le dossier qui contient `Cargo.toml`).

```text
puce8/
├── Cargo.toml
└── tests/
    ├── roms/            <- la collection de ROM de test (clone ci-dessous)
    │   ├── other/nestest.nes
    │   ├── other/nestest.log
    │   └── ...
    ├── catalogue.toml   <- liste des ROM, protocole, étape, résultat attendu (lu par `puce8-cli suite`)
    └── golden/          <- hash d'images de référence
```

Les chemins sont fixés dans le code : `tests/roms/` et `tests/catalogue.toml` (voir `crates/puce8-cli/src/suite.rs`), et `tests/roms/other/nestest.nes` / `nestest.log` (tests de `puce8-core`).

## Comment les télécharger

Collection publique `nes-test-roms` (blargg, kevtris, etc.), rassemblée par Christopher Pow :

```sh
git clone --depth 1 https://github.com/christopherpow/nes-test-roms.git tests/roms
```

Vérifier que ces trois fichiers existent :

- `tests/roms/other/nestest.nes`
- `tests/roms/other/nestest.log` (8 991 lignes)
- `tests/roms/instr_test-v5/rom_singles/01-basics.nes`

`tests/catalogue.toml` et `tests/golden/` ne font pas partie de ce clone : ils sont produits au fil du développement. Pour en garder une trace, les copier ailleurs (ou les versionner à part).

## Dossiers utilisés

| Dossier de `tests/roms/` | Étapes | Remarque |
|---|---|---|
| `other` (nestest) | E10, E11, E18 | trace CPU de référence |
| `instr_test-v5` | E14, E18, E26 | `official_only` et `all_instrs` = MMC1 |
| `instr_misc` | E14, E17, E30 | |
| `cpu_reset` | E14 | |
| `blargg_ppu_tests_2005.09.15b` | E15 à E17 | |
| `oam_read`, `oam_stress` | E16 | `oam_stress` optionnel |
| `ppu_vbl_nmi` | E17, E18, E36 | |
| `full_palette` | E19 | capture |
| `sprite_overflow_tests` | E20, E36 | |
| `sprite_hit_tests_2005.10.05` | E21, E36 | |
| `spritecans-2011` | E21 | capture |
| `ppu_open_bus` | E22 | optionnel |
| `cpu_dummy_reads`, `ppu_read_buffer` | E25 | optionnels |
| `scrolltest`, `MMC1_A12` | E26 | captures, optionnelles |
| `mmc3_test_2` | E27, E28 | `6-MMC3_alt` : échec attendu |
| `instr_timing`, `cpu_interrupts_v2` | E30, E36 | |
| `apu_test` | E30, E33, E35 | |
| `apu_mixer` | E34 | mesure par `wav-stats` |
| `apu_reset` | E35 | |
| `vbl_nmi_timing`, `branch_timing_tests`, `cpu_dummy_writes`, `sprdma_and_dmc_dma`, `dmc_dma_during_read4`, `read_joy3` | E36 | précision, plusieurs optionnels |

## Lancer les tests

```sh
cargo test --workspace                                        # tests unitaires (ceux qui lisent nestest sont ignorés si la ROM manque)
cargo run --release -p puce8-cli -- suite --jusqua E36        # suite de ROM (catalogue)
cargo run --release -p puce8-cli -- blargg tests/roms/instr_test-v5/official_only.nes --max-frames 6000
```

## Vérification (remplace `scripts/`, ignoré par git)

Même contrôle que `scripts/verifier.ps1` / `verifier.sh` :

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Plus une règle du projet : aucun caractère non ASCII dans `crates/**/*.rs` et `crates/**/*.toml`.

## Jeux

Aucune ROM commerciale n'est fournie ni versionnée. Pour tester un jeu, utiliser une copie de votre propre cartouche.
