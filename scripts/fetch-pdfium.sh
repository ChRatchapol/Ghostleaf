#!/usr/bin/env bash
# Downloads the pinned PDFium build into vendor/pdfium/<platform>/.
# One-time build-time download; the Ghostleaf app itself never uses the network.
set -euo pipefail
VERSION="${1:-8152}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)  PKG=pdfium-linux-x64;   DIR=linux-x64; LIB=lib/libpdfium.so ;;
  Linux-aarch64) PKG=pdfium-linux-arm64; DIR=linux-arm64; LIB=lib/libpdfium.so ;;
  Darwin-arm64)  PKG=pdfium-mac-arm64;   DIR=macos; LIB=lib/libpdfium.dylib ;;
  Darwin-x86_64) PKG=pdfium-mac-x64;     DIR=macos; LIB=lib/libpdfium.dylib ;;
  *) echo "unsupported platform"; exit 1 ;;
esac
TMP="$(mktemp -d)"
curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${VERSION}/${PKG}.tgz" | tar -xz -C "$TMP"
mkdir -p "$ROOT/vendor/pdfium/$DIR"
cp "$TMP/$LIB" "$ROOT/vendor/pdfium/$DIR/"
echo "PDFium $VERSION -> vendor/pdfium/$DIR"
