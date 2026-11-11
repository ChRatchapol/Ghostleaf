#!/usr/bin/env bash
# Build the Android APK (arm64, Android 8.0+).
#
# Needs: Rust (rustup), the Android SDK + NDK (ANDROID_HOME, ANDROID_NDK_HOME),
# Java 17+, and `cargo install cargo-ndk`. CI runs exactly this script.
#
#   bash scripts/build-android.sh            # release APK (debug-signed until phase 5)
#   bash scripts/build-android.sh debug      # debug build of the Rust code
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
PROFILE="${1:-release}"
PDFIUM_VERSION="8152"
JNI="android/app/src/main/jniLibs"
mkdir -p "$JNI/arm64-v8a"

rustup target add aarch64-linux-android

# PDF engine for Android (one-time, build-time download; the app never uses the network)
if [ ! -f "$JNI/arm64-v8a/libpdfium.so" ]; then
  TMP="$(mktemp -d)"
  curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${PDFIUM_VERSION}/pdfium-android-arm64.tgz" | tar -xz -C "$TMP"
  cp "$TMP/lib/libpdfium.so" "$JNI/arm64-v8a/"
fi

# libghostleaf.so
CARGO_FLAGS=()
[ "$PROFILE" = "release" ] && CARGO_FLAGS+=(--release)
cargo ndk -t arm64-v8a --platform 26 -o "$JNI" build -p ghostleaf-android "${CARGO_FLAGS[@]}"

# APK
cd android
if [ -x ./gradlew ]; then GRADLE=./gradlew; else GRADLE="bash ./gradlew"; fi
$GRADLE --no-daemon assembleRelease
VERSION="$(grep -m1 '^version = ' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
mkdir -p "$ROOT/dist"
cp app/build/outputs/apk/release/app-release.apk "$ROOT/dist/Ghostleaf-$VERSION-android-arm64.apk"
ls -la "$ROOT/dist/Ghostleaf-$VERSION-android-arm64.apk"
