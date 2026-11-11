# Android

Status: **phase 2**. Same app as on desktop: library, PDF/TXT/CSV reading,
page curl, sounds, themes, stealth (covers the page when you leave the app),
the system file picker and "Open with Ghostleaf".

- arm64 (`arm64-v8a`) only, Android 8.0+ (API 26)
- The manifest requests **no permissions at all**, not even `INTERNET`, so
  Android itself guarantees Ghostleaf cannot use the network.
- Data (settings, library, covers) lives in the app's private storage and is
  removed when the app is uninstalled.

## Getting the APK

Every push builds one in CI: GitHub → **Actions** → *CI* run → **Artifacts** →
`Ghostleaf-android-arm64` (a zip containing the `.apk`).

Install it by copying it to the phone and opening it (allow "install unknown
apps" for your file manager), or with `adb install Ghostleaf-*.apk`.

Until phase 5 the APK is signed with a debug key, so a later properly signed
version will need an uninstall first.

## Adding books

- **Open…** in the library shows Android's file picker (pick several at once).
- Or use **Open with → Ghostleaf** / **Share** from a file manager, browser downloads, mail, etc.
- Books are **not copied**: Ghostleaf keeps a read permission for each picked
  file and reopens it from where it is.
- Also scanned at start-up: `Android/data/app.ghostleaf/files/Books/` (handy over USB).

## Building locally

Needs Rust (rustup), Android SDK + NDK, Java 17+, and `cargo install cargo-ndk`:

```bash
export ANDROID_HOME=…/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
bash scripts/build-android.sh        # → dist/Ghostleaf-<version>-android-arm64.apk
```

Logs: `adb logcat -s ghostleaf`.

## How it fits together

```
android/                     Gradle project: manifest, icons, MainActivity.java
                             (file picker, "Open with", system-bar colours)
  app/src/main/jniLibs/      filled by the build: libghostleaf.so + libpdfium.so
crates/gl-android            android_main() → libghostleaf.so
app/  (ghostleaf_app lib)    the whole app, shared with desktop; desktop-only parts
                             (file dialog, boss key, "open folder") are cfg-gated
```

## Plan

1. ✅ Gradle project, Android entry point, APK in CI
2. ✅ File picker, "Open with Ghostleaf", back button, phone layout, system bars, touch stealth
3. Polish on devices: tablets/landscape spread, soft keyboard for "go to page", edge-to-edge (target API 35)
4. More mobile stealth: blurred recents thumbnail, optional screenshot block, two-finger tap veil
5. Release signing (keystore in CI secrets) and the APK attached to GitHub Releases
