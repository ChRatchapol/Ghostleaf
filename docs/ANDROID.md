# Android

Status: **phase 1**. The app starts on Android and shows your library; the
system file picker and phone-specific UI come next (see the plan below).

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

## Adding books (phase 1)

Until the file picker lands, Ghostleaf adds every PDF/TXT/CSV it finds in its
own folder at start-up:

```
Internal storage/Android/data/app.ghostleaf/files/Books/
```

Connect the phone to a computer over USB (File transfer), open that folder,
copy books in, then start Ghostleaf. (On the phone itself most file managers
can't open `Android/data` on Android 11+; the computer can.)

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
android/                     Gradle project (no Java/Kotlin yet): manifest, icons
  app/src/main/jniLibs/      filled by the build: libghostleaf.so + libpdfium.so
crates/gl-android            android_main() → libghostleaf.so
app/  (ghostleaf_app lib)    the whole app, shared with desktop; desktop-only parts
                             (file dialog, boss key, "open folder") are cfg-gated
```

## Plan

1. ✅ Gradle project, Android entry point, APK in CI, library from the Books folder
2. Android file picker (Storage Access Framework) + "Open with Ghostleaf" + back button
3. Phone layout: portrait single page / landscape spread, safe areas, touch targets
4. Mobile stealth: hide on app switch, blurred recents, optional screenshot block, two-finger tap veil
5. Release signing (keystore in CI secrets) and the APK attached to GitHub Releases
