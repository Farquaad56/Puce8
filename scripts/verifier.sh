#!/bin/sh
# T0 + T2 : fmt, clippy, tests. S'arrête au premier échec (code de sortie non nul).
set -e
if grep -rnP '[^\x00-\x7F]' crates --include='*.rs' --include='*.toml'; then echo "FAIL: caracteres non ASCII"; exit 1; fi
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
