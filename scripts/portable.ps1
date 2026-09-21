# Portable Windows zip: rimsort-rs.exe + portable.txt (the marker makes it keep all data in .\data).
# Usage: scripts/portable.ps1 [-Exe target/release/rimsort-rs.exe] [-Out target/release/bundle/portable]
param(
  [string]$Exe = 'target/release/rimsort-rs.exe',
  [string]$Out = 'target/release/bundle/portable'
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Exe)) { throw "$Exe not found - run 'cargo tauri build --no-bundle' first" }
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$stage = Join-Path $Out 'RimSort-rs'
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $Exe (Join-Path $stage 'rimsort-rs.exe')
Set-Content (Join-Path $stage 'portable.txt') "Delete this file to store settings in %LOCALAPPDATA%\RimSort-rs instead of the data folder next to the exe."
$zip = Join-Path $Out "RimSort-rs_${version}_x64-portable.zip"
Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $stage -DestinationPath $zip
Write-Output $zip
