# Builds a release and assembles a portable folder + zip in dist/.
#   powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$dll = "vendor/pdfium/windows-x64/pdfium.dll"
if (-not (Test-Path $dll)) { & "$PSScriptRoot/fetch-pdfium.ps1" }
cargo build --release -p ghostleaf
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches.Groups[1].Value
$out = "dist/Ghostleaf-$version-windows-x64"
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item target/release/ghostleaf.exe $out
Copy-Item $dll $out
Copy-Item README.md, PRIVACY.md, THIRD_PARTY.md, LICENSE-MIT, LICENSE-APACHE $out
Copy-Item vendor/pdfium/LICENSE "$out/PDFIUM-LICENSE.txt"
New-Item -ItemType Directory -Force -Path "$out/licenses" | Out-Null
Copy-Item app/assets/fonts/licenses/* "$out/licenses/"
Compress-Archive -Path "$out/*" -DestinationPath "$out.zip" -Force
Write-Host "Done: $out.zip"
Write-Host "Tip: create an empty 'portable.flag' next to ghostleaf.exe for portable mode."
