# T0 + T2 : fmt, clippy, tests. S'arrête au premier échec (code de sortie non nul).
$ErrorActionPreference = "Stop"

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: cargo fmt"; exit $LASTEXITCODE }

cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: cargo clippy"; exit $LASTEXITCODE }

cargo test --workspace
exit $LASTEXITCODE
