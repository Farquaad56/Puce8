#!/bin/sh
# T0 + T2 : fmt, clippy, tests. S'arrête au premier échec (code de sortie non nul).
set -e
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
