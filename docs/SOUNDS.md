# Page sounds

Ghostleaf plays three kinds of paper sounds:

| Kind | When | Files |
|---|---|---|
| **flip** | A full page turn: click, arrow keys, mouse wheel | `flip-1..3.wav` |
| **peel** | You start dragging a page by hand | `peel-1..3.wav` |
| **land** | A dragged page lands after you let go | `land-1..2.wav` |

## Recorded sounds (local builds)

The recorded sounds are cut from the **Sonniss GDC Game Audio Bundle**. They are
royalty-free for use in products, but the raw files may not be redistributed,
so **they are not in this repository**. To build with them:

```powershell
pip install numpy          # and have ffmpeg on PATH
python scripts\prepare-sounds.py "E:\Sonniss-GDC"
cargo build --release      # build.rs embeds whatever is in app/assets/sounds/
```

Sources (pack → file):
- Touchdown Audio – *The White Stuff*: `CATALOGUE_Page_Turn_9`, `NOTEPAD_Page_Turn_4`
- The Soundcatcher – *Paperlife*: `Paper_Pergament_Pages_Turn_Flip_20`
- PMSFX – *Books Alive*: `PM_BA_BOOK_6_14 Book, Paper, Page Turn`
- 344 Audio – *Antique Books*: `Slow Page Turns 6` (two slices)
- Cinematic Sound Design – *Paper Foley*: `Encyclopedia Glossy Page Turn Muted`
- CB Sounddesign – *Infographic SFX*: `Page_Turn_14`

Each clip is converted to mono 44.1 kHz 16-bit, trimmed of silence, faded and
loudness-matched, so all variants sit at the same level.

## Fallback

Without those files (e.g. CI builds from this repo) the app synthesizes paper
sounds in code (`app/src/sound.rs`). Nothing is downloaded at runtime.
