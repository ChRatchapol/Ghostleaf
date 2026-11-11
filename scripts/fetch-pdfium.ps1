# Downloads the pinned PDFium build into vendor/pdfium/windows-x64/pdfium.dll.
# This is a one-time *build-time* download. The Ghostleaf app itself never uses the network.
param([string]$Version = "8152", [string]$Arch = "x64")
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "vendor/pdfium/windows-$Arch"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
$url = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F$Version/pdfium-win-$Arch.tgz"
$tmp = Join-Path $env:TEMP "pdfium-$Version-$Arch"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
Write-Host "Downloading $url"
Invoke-WebRequest -Uri $url -OutFile "$tmp/pdfium.tgz"
tar -xzf "$tmp/pdfium.tgz" -C $tmp
Copy-Item "$tmp/bin/pdfium.dll" $dest -Force
Write-Host "PDFium $Version -> $dest\pdfium.dll"
