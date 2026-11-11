# Releasing

## One-time setup: recorded sounds for release builds

The recorded page sounds (see [SOUNDS.md](SOUNDS.md)) may be used inside the app
but not shared as raw files, and GitHub secrets are limited to 48 KB. So the
clips are committed **encrypted**, and only the passphrase is a secret.

1. Make sure `app/assets/sounds/*.wav` exist (`python scripts/prepare-sounds.py "E:\Sonniss-GDC"`).
2. Pick a long random passphrase, then in **Git Bash**:
   ```bash
   SOUNDS_PASSPHRASE='…your passphrase…' bash scripts/encrypt-sounds.sh
   git add app/assets/sounds.tar.gz.gpg && git commit -m "Add encrypted release sounds"
   ```
3. On GitHub: **Settings → Secrets and variables → Actions → New repository secret**,
   name `SOUNDS_PASSPHRASE`, value = the same passphrase.

Without the secret (forks, pull requests) builds still work, with synthesized sounds.

## Making a release

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`) and commit.
2. Tag and push:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The **Release** workflow checks that the tag matches the version, runs the
   no-network check and tests, builds:
   - `Ghostleaf-<version>-windows-x64.zip`: `ghostleaf.exe` + `pdfium.dll` + licences
   - `Ghostleaf-<version>-linux-x64.tar.gz`: `ghostleaf` + `libpdfium.so` + `.desktop` + icon + licences
   - `SHA256SUMS.txt`
4. It creates a **draft** release. Check it on GitHub → Releases, edit the notes, then **Publish**.

To re-run for an existing tag: Actions → Release → *Run workflow* → enter the tag.
