<#
.SYNOPSIS
  Build LightCraft Film Scan for Windows x64 as a portable ZIP.
.DESCRIPTION
  The GitHub workflow .github/workflows/fotofieber-build.yml runs it; it also works on any Windows
  machine with Rust (MSVC) and git.

    pwsh packaging/fotofieber/build-windows.ps1

  Output in dist\fotofieber\: lightcraft-filmscan-<version>-windows-x64.zip, a folder
  "LightCraft Film Scan" with lightcraft-filmscan.exe, lightcraft-filmscan-cli.exe and the licences.

  There is no MSI on purpose: upstream's installer (packaging/windows/lightcraft.wxs) has the
  official LightCraft's upgrade code, registry keys and folder, so it would replace an installed
  LightCraft. LIGHTCRAFT_FILMSCAN=1 gives the binaries their own settings folder, library and app id
  (crates/engine/src/flavor.rs). The binaries are not signed: SmartScreen asks before the first
  start. The C runtime is linked statically, as upstream does, so no VC++ redistributable is needed.
#>
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Out = Join-Path $Root 'dist\fotofieber'
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Target = 'x86_64-pc-windows-msvc'

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

$Version = $null
$inPkg = $false
foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
  if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
  if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }
$Sha = (git -C $Root rev-parse HEAD)
$env:LIGHTCRAFT_BUILD_SHA = $Sha
if (-not $env:LIGHTCRAFT_BUILD_DATE) { $env:LIGHTCRAFT_BUILD_DATE = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd') }
$env:LIGHTCRAFT_FILMSCAN = '1'
Write-Output "LightCraft Film Scan $Version for Windows x64"

. (Join-Path $Root 'packaging\windows\fonts.ps1')
Initialize-CraftFonts $Root
$flagVar = 'CARGO_TARGET_' + ($Target.ToUpper() -replace '-', '_') + '_RUSTFLAGS'
[Environment]::SetEnvironmentVariable($flagVar, '-C target-feature=+crt-static')
$env:LIGHTCRAFT_REQUIRE_WINRES = '1'
Invoke-Native "cargo build ($Target)" { cargo build --release --locked -p lightcraft -p lightcraft-cli --features lightcraft/heif,lightcraft-cli/heif --target $Target }
$Bin = Join-Path $TargetDir "$Target\release"

$Stage = Join-Path $TargetDir 'fotofieber-windows'
$Folder = Join-Path $Stage 'LightCraft Film Scan'
Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Folder, $Out | Out-Null
# The same names as the Linux commands, so they are not mistaken for an official LightCraft.
Copy-Item (Join-Path $Bin 'lightcraft.exe') (Join-Path $Folder 'lightcraft-filmscan.exe')
Copy-Item (Join-Path $Bin 'lightcraft-cli.exe') (Join-Path $Folder 'lightcraft-filmscan-cli.exe')
# Our own package README, not the repository's: README.md can then change after a build without making
# the package stale (publish.sh leaves README.md out of its source check).
Copy-Item (Join-Path $Root 'packaging\fotofieber\README-package.md') (Join-Path $Folder 'README.md')
foreach ($f in 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Folder }
}
. (Join-Path $Root 'packaging\windows\font-licences.ps1')
Write-FontLicences (Join-Path $Folder 'FONT-LICENCES.txt') (Join-Path $Root 'assets\fonts\OFL-Inter.txt') $env:CRAFT_FONTS_DIR
Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt') -ErrorAction SilentlyContinue | ForEach-Object {
  Copy-Item $_.FullName (Join-Path $Folder "OFL-$($_.Directory.Name).txt")
}

# Smoke test: the command-line tool runs and renders the demo library with the embedded fonts.
Invoke-Native 'lightcraft-filmscan-cli --version' { & (Join-Path $Folder 'lightcraft-filmscan-cli.exe') --version }
$Snapshot = Join-Path $Stage 'smoke.png'
Invoke-Native 'lightcraft-filmscan-cli snapshot --demo' { & (Join-Path $Folder 'lightcraft-filmscan-cli.exe') snapshot --demo -o $Snapshot --size 1280x800 }
if (-not (Test-Path $Snapshot) -or (Get-Item $Snapshot).Length -lt 10000) { throw 'the demo snapshot was not written' }

$Zip = Join-Path $Out "lightcraft-filmscan-$Version-windows-x64.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Folder -DestinationPath $Zip
Set-Content -Path (Join-Path $Out 'BUILT_FROM-windows') -Value $Sha -NoNewline
Get-FileHash -Algorithm SHA256 $Zip | Format-List Hash, Path
