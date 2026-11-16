# Windows release checklist

Run this on the Windows build from CI (Actions → latest run → the Windows
artifact) before tagging a release. Tick each line or note what went wrong.

## Start-up
- [ ] `ghostleaf.exe` starts by double-click; no console window appears.
- [ ] The exe, the taskbar and the window all show the leaf icon.
- [ ] Text and icons are sharp at your display scaling (try 100% and 150%, and drag the window to a second monitor if you have one).

## Opening and reading
- [ ] Open a PDF, a TXT and a CSV via **Open…** and by drag and drop.
- [ ] "Open with → Ghostleaf" from Explorer opens the file.
- [ ] A large PDF (300+ pages) opens quickly, turns smoothly, and memory stays reasonable in Task Manager.
- [ ] A Thai TXT file wraps at word boundaries.
- [ ] Close and reopen a book: it resumes on the same page.

## Page turning
- [ ] Curl from a top corner, a bottom corner and the middle of the edge; forward and back.
- [ ] Spread and single-page modes; right-to-left; first blank page toggle.
- [ ] Slide and None styles.
- [ ] Sounds play; `M` mutes and unmutes.

## Stealth
- [ ] Mouse leaves the window: the page hides after the delay; comes back on return.
- [ ] **Minimize** style: the window minimizes and can be restored.
- [ ] **Boss key** `Ctrl+Alt+H` hides and shows from another app.
- [ ] Always on top works.
- [ ] No hide while dragging a page or while the Open dialog is up.

## Look
- [ ] Each theme, plus a custom tint; the title bar follows in *Match theme*.
- [ ] *Hidden* title bar: drag to move, resize from edges, minimize / maximize / close.
- [ ] Settings scroll properly in a small window.
- [ ] Switch the language to Thai and back; nothing is cut off.

## Privacy
- [ ] The Privacy screen lists the stored files; **Forget this book** and **Wipe everything** work.
- [ ] Portable mode: with `portable.flag` next to the exe, data is written beside it and not in AppData.
