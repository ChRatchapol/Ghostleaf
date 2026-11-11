#!/usr/bin/env bash
# Restore the recorded page sounds for a release build (used by CI).
# Without SOUNDS_PASSPHRASE (forks, PRs) it does nothing and the build falls
# back to synthesized sounds.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENC="$ROOT/app/assets/sounds.tar.gz.gpg"
if [ -z "${SOUNDS_PASSPHRASE:-}" ] || [ ! -f "$ENC" ]; then
  echo "Recorded sounds not available; using synthesized sounds."
  exit 0
fi
# the folder only holds git-ignored files, so a fresh checkout doesn't have it
mkdir -p "$ROOT/app/assets/sounds"
gpg --batch --yes --pinentry-mode loopback --decrypt --passphrase "$SOUNDS_PASSPHRASE" "$ENC" \
  | tar -xzf - -C "$ROOT/app/assets/sounds"
echo "Recorded sounds restored: $(ls "$ROOT/app/assets/sounds"/*.wav | wc -l) files"
