#!/usr/bin/env bash
# Build a release and pack dist/Ghostleaf-<version>-linux-x64.tar.gz
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
[ -f vendor/pdfium/linux-x64/libpdfium.so ] || bash scripts/fetch-pdfium.sh
cargo build --release -p ghostleaf
VERSION="$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)"
NAME="Ghostleaf-$VERSION-linux-x64"
OUT="dist/$NAME"
rm -rf "$OUT" && mkdir -p "$OUT/licenses"
cp target/release/ghostleaf vendor/pdfium/linux-x64/libpdfium.so "$OUT/"
cp README.md PRIVACY.md THIRD_PARTY.md LICENSE-MIT LICENSE-APACHE "$OUT/"
cp vendor/pdfium/LICENSE "$OUT/PDFIUM-LICENSE.txt"
cp app/assets/fonts/licenses/* "$OUT/licenses/"
cp app/assets/icons/icon-256.png "$OUT/ghostleaf.png"
cat > "$OUT/ghostleaf.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Ghostleaf
Comment=Private offline reader
Exec=ghostleaf %f
Icon=ghostleaf
Categories=Office;Viewer;
MimeType=application/pdf;text/plain;text/csv;text/tab-separated-values;
DESKTOP
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
echo "Done: dist/$NAME.tar.gz"
