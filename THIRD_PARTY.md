# Third-party components

| Component | Use | Licence |
|---|---|---|
| [PDFium](https://pdfium.googlesource.com/pdfium/) (prebuilt by [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)) | PDF rendering | BSD-3-Clause / Apache-2.0 — see `vendor/pdfium/LICENSE` and `vendor/pdfium/licenses/` |
| [Inter](https://github.com/rsms/inter) | Interface font | SIL OFL 1.1 — `app/assets/fonts/licenses/OFL-inter.txt` |
| [Literata](https://github.com/googlefonts/literata) | Default reading font | SIL OFL 1.1 — `app/assets/fonts/licenses/OFL-literata.txt` |
| [Noto Sans Thai / Noto Serif Thai](https://github.com/notofonts/thai) | Thai text | SIL OFL 1.1 — `app/assets/fonts/licenses/OFL-notosansthai.txt`, `OFL-notoserifthai.txt` |
| [MuseoModerno](https://github.com/Omnibus-Type/MuseoModerno) | "ghostleaf" wordmark only | SIL OFL 1.1 — `app/assets/fonts/licenses/OFL-museomoderno.txt` |

The bundled font files are static instances (fixed weight) generated from the
variable fonts with fontTools; they are unmodified otherwise.

Rust crate dependencies and their licences are listed in `Cargo.lock`; the
allowed licence set is enforced by `deny.toml`. Paper sounds: local builds may embed clips cut from the Sonniss GDC Game Audio Bundle (royalty-free for use in products; raw files are not redistributed, see docs/SOUNDS.md). Otherwise sounds are synthesized in code (`app/src/sound.rs`).
