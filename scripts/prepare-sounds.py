#!/usr/bin/env python3
"""Build Ghostleaf's page sounds from a local Sonniss GDC Game Audio Bundle.

    python scripts/prepare-sounds.py "E:/Sonniss-GDC"

Requires ffmpeg on PATH and numpy. Writes app/assets/sounds/{flip,peel,land}-N.wav
(mono, 44.1 kHz, 16-bit, trimmed, level-matched). These files are git-ignored:
the Sonniss licence allows use inside products but not redistribution of the raw
sounds. Builds without them fall back to synthesized paper sounds.
"""
import subprocess, sys, tempfile, wave, pathlib
import numpy as np

SR = 44100
ROOT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "E:/Sonniss-GDC")
OUT = pathlib.Path(__file__).resolve().parent.parent / "app" / "assets" / "sounds"

# (output name, source path inside the bundle, start s, end s or None, peak dB)
CLIPS = [
    ("flip-1", "sonniss-2019/**/Touchdown Audio -The White Stuff/CATALOGUE_Page_Turn_9.wav", 0.0, None, -4),
    ("flip-2", "sonniss-2017/**/The Soundcatcher - Paperlife/Paper_Pergament_Pages_Turn_Flip_20.wav", 0.15, None, -4),
    ("flip-3", "sonniss-2019/**/Touchdown Audio -The White Stuff/NOTEPAD_Page_Turn_4.wav", 0.2, None, -4),
    ("peel-1", "sonniss-2019/**/PMSFX - Books Alive/PM_BA_BOOK_6_14 Book, Paper, Page Turn.wav", 0.05, 0.86, -4),
    ("peel-2", "sonniss-2026/**/344 Audio - Antique Books/PAPRMisc_Antique Books Slow Page Turns 6_344 Audio_Antiques - Books.wav", 4.2, 5.0, -4),
    ("peel-3", "sonniss-2026/**/344 Audio - Antique Books/PAPRMisc_Antique Books Slow Page Turns 6_344 Audio_Antiques - Books.wav", 9.6, 10.4, -4),
    ("land-1", "sonniss-2026/**/Cinematic Sound Design - Paper Foley/Encyclopedia Glossy Page Turn Muted.wav", 0.08, None, -6),
    ("land-2", "sonniss-2020/**/CB Sounddesign - Infographic SFX/Page_Turn_14.wav", 0.05, 0.4, -6),
]


def decode(src: pathlib.Path) -> np.ndarray:
    """Decode to mono 44.1 kHz float with leading/trailing silence removed."""
    with tempfile.TemporaryDirectory() as d:
        tmp = pathlib.Path(d) / "x.wav"
        af = "silenceremove=start_periods=1:start_threshold=-55dB,areverse,silenceremove=start_periods=1:start_threshold=-60dB,areverse"
        subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-i", str(src), "-ac", "1", "-ar", str(SR), "-af", af, "-c:a", "pcm_s16le", str(tmp)], check=True)
        with wave.open(str(tmp)) as w:
            return np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float32) / 32768


def save(name: str, x: np.ndarray, peak_db: float) -> None:
    fi, fo = int(0.008 * SR), int(0.06 * SR)
    x = x.copy()
    x[:fi] *= np.linspace(0, 1, fi)
    x[-fo:] *= np.linspace(1, 0, fo) ** 2
    x *= 10 ** (-24 / 20) / max(float(np.sqrt(np.mean(x**2))), 1e-6)  # match loudness (RMS)
    lim = 10 ** (peak_db / 20)
    if np.abs(x).max() > lim:
        x *= lim / np.abs(x).max()
    with wave.open(str(OUT / f"{name}.wav"), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((x * 32767).astype(np.int16).tobytes())
    print(f"{name}.wav  {len(x) / SR:.2f}s")


OUT.mkdir(parents=True, exist_ok=True)
for name, pattern, a, b, peak in CLIPS:
    found = sorted(ROOT.glob(pattern))
    if not found:
        print(f"skip {name}: not found ({pattern})")
        continue
    x = decode(found[0])
    save(name, x[int(a * SR): int(b * SR) if b else None], peak)
