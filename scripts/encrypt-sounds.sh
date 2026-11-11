#!/usr/bin/env bash
# Pack app/assets/sounds/*.wav into an encrypted archive that is safe to commit.
# Only CI (with the SOUNDS_PASSPHRASE secret) can decrypt it for release builds.
#
#   SOUNDS_PASSPHRASE='your long passphrase' bash scripts/encrypt-sounds.sh
#
# Works in Git Bash on Windows (gpg ships with Git for Windows).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
: "${SOUNDS_PASSPHRASE:?set SOUNDS_PASSPHRASE first}"
cd "$ROOT/app/assets/sounds"
ls flip-*.wav peel-*.wav land-*.wav >/dev/null
tar -czf - flip-*.wav peel-*.wav land-*.wav \
  | gpg --batch --yes --pinentry-mode loopback --symmetric --cipher-algo AES256 \
        --passphrase "$SOUNDS_PASSPHRASE" -o "$ROOT/app/assets/sounds.tar.gz.gpg"
echo "Wrote app/assets/sounds.tar.gz.gpg ($(wc -c < "$ROOT/app/assets/sounds.tar.gz.gpg") bytes)"
