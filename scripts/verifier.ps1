# T0 + T2 : fmt, clippy, tests. S'arrête au premier échec (code de sortie non nul).
$ErrorActionPreference = "Stop"

$bad = Get-ChildItem crates -Recurse -Include *.rs, *.toml | Select-String -Pattern '[^\x00-\x7F]'
if ($bad) { $bad | Select-Object -First 20; Write-Host "FAIL: caracteres non ASCII"; exit 1 }

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: cargo fmt"; exit $LASTEXITCODE }

cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { Write-Host "FAIL: cargo clippy"; exit $LASTEXITCODE }

cargo test --workspace
exit $LASTEXITCODE
