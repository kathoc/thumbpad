param([string]$Iscc = '', [switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (!$SkipChecks) {
  cargo fmt --check
  if ($LASTEXITCODE) { throw 'Formatting check failed' }
  cargo clippy --locked --all-targets -- -D warnings
  if ($LASTEXITCODE) { throw 'Clippy failed' }
  cargo test --locked
  if ($LASTEXITCODE) { throw 'Tests failed' }
}
cargo build --release --locked
if ($LASTEXITCODE) { throw 'Release build failed' }
if (!$Iscc) {
  $candidates = @(
    (Join-Path (Get-Location) 'tools\inno\ISCC.exe'),
    'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
    'C:\Program Files\Inno Setup 7\ISCC.exe'
  )
  $Iscc = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
  if (!$Iscc) { $Iscc = (Get-Command ISCC.exe -ErrorAction Stop).Source }
}
$version = (Get-Content Cargo.toml | Select-String '^version = "([^"]+)"').Matches[0].Groups[1].Value
& $Iscc "/DAppVersion=$version" 'installer\thumbpad.iss'
if ($LASTEXITCODE) { throw 'Installer compilation failed' }
Copy-Item -LiteralPath target/release/thumbpad.exe -Destination dist/thumbpad-x64.exe
$lines = Get-ChildItem dist/*.exe | ForEach-Object {
  $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  "$hash  $($_.Name)"
}
$lines | Set-Content -Encoding ascii dist/SHA256SUMS.txt
Write-Host "Built Thumbpad $version in dist/"
