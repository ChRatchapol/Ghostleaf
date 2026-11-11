# Privacy

Ghostleaf is built to be **100% offline and private**, in a way you can check.

## What Ghostleaf never does
- It never connects to the internet. The program contains **no networking code**:
  no HTTP client, no TLS, no sockets. This is enforced in CI with
  [`cargo-deny`](deny.toml), which fails the build if any networking crate appears.
- No accounts, no analytics or telemetry, no crash reporting, no ads, no update checks.
- It never copies, uploads or analyses your books. PDFs are rendered on your
  computer by PDFium; text and CSV files are read from where you keep them.
- It never follows links inside documents.

## What Ghostleaf stores (all of it)
Everything lives in **one folder**, as plain files you can open and read:

| File | Contents | Why |
|---|---|---|
| `settings.json` | Your preferences (theme, fonts, stealth options…) | Remember your setup |
| `library.json` | For each book: a content id, the file's path, its title, format, reading position, bookmarks, per-book direction / blank-page choice, dates added and last opened | Show your library and resume where you left off |
| `covers/<id>.png` | A small (≤ 240 px wide) thumbnail of a PDF's first page. **Optional**: turn off "Keep small cover thumbnails" | Show covers on the shelf |

Nothing else is written: no book text, no full-size pages, no reading statistics,
no search history, nothing about your device. Page images exist only in memory
while a book is open.

**Where:**
- Windows: `%APPDATA%\Ghostleaf\Ghostleaf\data`
- macOS: `~/Library/Application Support/app.Ghostleaf.Ghostleaf`
- Linux: `~/.local/share/ghostleaf`
- **Portable mode:** put an empty file named `portable.flag` next to the app and
  everything is stored in `ghostleaf-data/` beside it instead (handy on a USB stick).

The book id is a BLAKE3 hash of the file size and its first/last 64 KiB, so
Ghostleaf can recognise a book after it has been moved or renamed. It is not
derived from anything about you.

## You are in control
Open **Privacy** in the app to see the data folder and every stored file with its
size, and to:
- remove a single book (✕ on its cover),
- delete all cover thumbnails,
- forget all books,
- erase everything (settings included).

Uninstalling the app and deleting the folder above removes every trace.
