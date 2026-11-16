# Ghostleaf — Product & Technical Plan (v0.4)

> Working name: **Ghostleaf** (see §11 for alternatives). A private, offline, Rust-native e-reader
> that combines stealth reading, physical-feeling 3D page curl, and a 3D bookshelf —
> with Thorium Reader's reading quality as the floor.

---

## 0. Decisions (updated 2026-11-11)
| Topic | Decision |
|---|---|
| Name | **Ghostleaf** |
| Logo | **A · Leaf-fold** (minimal monochrome mark) |
| Typography | Name/wordmark: **MuseoModerno Italic** (only for the name/logo). UI: Inter. Reading: Literata (serif) / Inter (sans) + Noto Serif/Sans Thai. PDFs always render with their own embedded fonts |
| Repository | https://github.com/ChRatchapol/Ghostleaf |
| Licence | **Open source**: MIT OR Apache-2.0 |
| v1 scope | **Windows only; PDF, TXT, CSV** |
| UI languages | **English + Thai** at launch |
| Web app | Later; it is also the fallback for iPhone/iPad if a native iOS build isn't possible (see below) |
| Mobile | **Android in progress** (arm64, min Android 8 / API 26), see §6.1 |
| Release order | Desktop v0.1.0 → Android phases 3–5 → Markdown → macOS/Linux packages → EPUB/CBZ → MOBI/AZW → 3D shelf + Thorium parity |
| Apple | **No paid Apple Developer account.** macOS is built and shipped unsigned (Gatekeeper warns on first open). iOS only if it can be built and installed without a paid account; otherwise iPhone/iPad users get the web app |
| App stores | **Build-only for now:** GitHub Releases (Windows, Linux, Android APK). Google Play maybe later; F-Droid optional |
| Markdown | **Required:** read `.md` / `.markdown` files (added 2026-11-11), see §2.3 and roadmap Step 5 |
| Biometric unlock | **Yes** (Android stealth reveal; adds the `USE_BIOMETRIC` permission only, no network) |
| Who builds | Claude builds all of it; owner reviews and runs the Windows builds |

## 1. Goals and non-goals

### Goals
1. **One Rust codebase** that builds natively for **Windows (primary, v1)**, then **macOS**, **Linux**, **Android**, **iOS**, and possibly a **web app** (decision deferred, not in v1).
2. **Zero network by construction.** The app never opens a socket. No telemetry, accounts, crash uploaders, update pings, remote fonts or ads.
3. **Store as little as possible, and show the user all of it.** Every byte the app saves can be inspected and deleted from inside the app.
4. **Feel physical:** realistic 3D page curl with touch/mouse physics, paper sounds, 3D shelves.
5. **Read anything common:** TXT, **Markdown**, PDF, EPUB, MOBI, AZW, AZW3, CBZ (later CBR, FB2, DjVu).
6. **Instant stealth:** the reading surface hides the moment the cursor leaves the window.
7. **Reading quality at least equal to Thorium Reader** for EPUB/PDF (TOC, bookmarks, search, typography settings, accessibility).

### Non-goals (v1)
- DRM. No LCP, Adobe ADEPT or Kindle DRM. DRM-protected MOBI/AZW files are detected and rejected with a clear message.
- Cloud sync, store, OPDS catalogues (OPDS would need network, so it's out of v1; maybe a later opt-in plugin, off by default).
- Editing PDFs.

### Reference products
| Product | What we take | What we do better |
|---|---|---|
| **Hide Reader** (Rust, Windows/macOS, ~17 MB, paid membership) | Stealth hide-on-mouse-out, page-flip animation, same format list, deep text customization | Free/open, verifiable zero-network, 3D curl + shelves, web build, Linux |
| **Flip Library** (Microsoft Store) | 3D page curl from any corner, paper sounds, 3D shelves, LTR/RTL, single/spread, first blank page toggle, dark/light | Not PDF-only: all 7 formats; reflowable text; stealth |
| **Thorium Reader** (EDRLab, Electron) | Baseline feature set: EPUB 2/3 + fixed layout, PDF, TOC, bookmarks, annotations, full-text search, font/spacing/hyphenation/columns settings, sepia/night, screen-reader support | ~10× smaller, native speed (no Electron), stealth, physical page feel |

---

## 2. Feature specification

### 2.1 Stealth Reading
- **Trigger:** cursor leaves the window (`winit::WindowEvent::CursorLeft`; web: `mouseleave` on `document`). Optional extra triggers: window loses focus, and a **boss key** (global hotkey, default `Ctrl+Alt+H`).
- **Hide styles** (user choice):
  1. **Veil:** instant blur + solid overlay (theme colour) over the page. Default; works everywhere.
  2. **Decoy:** shows a neutral decoy screen (blank document / spreadsheet-like grid, all drawn locally).
  3. **Fade:** window opacity → 0 (Windows/macOS) and click-through.
  4. **Minimize:** minimize to taskbar / tray.
- **Reveal:** cursor re-enters (optional: require click, or a hold-delay of 0–500 ms to avoid accidental reveals).
- **Timing:** hide must happen within **1 frame (≤16 ms)**: the veil is a pre-built GPU layer, not a re-render.
- **Window extras:** always-on-top toggle, borderless "slim" mode, adjustable window opacity, hide from taskbar (Windows), no title text ("Ghostleaf" title can be replaced by a neutral title).
- **Edge cases:** don't trigger while dragging a page curl outside the window; don't trigger while a file dialog is open.

### 2.2 Page turning
| Mode | Description |
|---|---|
| **3D Curl** | Peel from any corner or edge; page bends around a moving cylinder; the back of the page shows a mirrored, dimmed copy (or the next page for thin paper); dynamic shadow on the page underneath. |
| **Slide** | Smooth left-swipe (or right in RTL) with inertia. |
| **Fade / None** | Accessibility and low-power option. |

- **Input:** mouse drag, touch/pen drag, trackpad two-finger swipe, wheel, keyboard (←/→, PgUp/PgDn, Space), click zones (left 30% / right 30%).
- **Physics:** fold position driven by a critically damped spring; on release, use fling velocity + position to decide complete vs. snap back; 60–144 Hz, frame-rate independent.
- **Spread mode:** in two-page view the right page curls over the left (mirrored in RTL).
- **Sound:** a set of 6–10 recorded paper sounds (CC0), randomized pitch/volume, separate sounds for slow peel vs. fast flip; **one-key mute** (`M`) plus a toolbar toggle; volume slider.

### 2.3 Formats
| Format | Type | Engine (planned) | Notes |
|---|---|---|---|
| PDF | Fixed | `pdfium-render` + bundled PDFium (BSD-licensed) | Fallback/WASM-light option: `hayro` (pure-Rust, Apache/MIT). JavaScript, XFA and V8 disabled. |
| EPUB 2/3 | Reflow + fixed layout | `zip` + `quick-xml` own parser (OPF, NCX/NAV, spine) → our layout engine | Font de-obfuscation (IDPF/Adobe). FXL → rendered as fixed pages. |
| MOBI / AZW | Reflow | own `mobi` module (PalmDOC/HUFF-CDIC decompression, EXTH metadata) | Evaluate the `mobi` and `ebook-rs` crates first; write our own if quality/licence isn't good enough. |
| AZW3 (KF8) | Reflow | own `kf8` module: extract KF8 HTML/CSS/images into an internal EPUB-like model | DRM flag in record 0 → friendly rejection. |
| CBZ | Fixed (images) | `zip` + `image` (JPEG/PNG/WebP/AVIF) | Reads `ComicInfo.xml` for title and manga RTL. |
| TXT | Reflow | `chardetng` + `encoding_rs` (UTF-8/16, TIS-620 Thai, GBK, Shift-JIS, Win-1252…) | Auto-detects chapter headings ("Chapter 1", "第1章", "บทที่ 1") to build a TOC. |
| **CSV / TSV** (v1) | Paginated table | `csv` + encoding detection | Auto-detects delimiter (`,` `;` tab `|`); header row repeated on every page; column widths fitted to page; long cells wrap. |
| **Markdown** (`.md`, `.markdown`) | Reflow (rich text) | `pulldown-cmark` (CommonMark + GFM tables, task lists, strikethrough, footnotes) → styled text through the same pipeline as TXT | Headings build the TOC; code blocks in a monospace font; tables laid out like CSV; **images only from local relative paths, never fetched from the web**; raw HTML shown as text, never run; links open only after a confirm dialog. Theme colours and typography settings apply as for TXT. |
| Later | — | CBR (`unrar` licence check), FB2, DjVu | "More formats coming". |

All formats implement one trait:
```rust
pub trait Document: Send {
    fn metadata(&self) -> &Metadata;            // title, authors, lang, direction
    fn toc(&self) -> &[TocEntry];
    fn kind(&self) -> Kind;                      // Fixed | Reflowable
    fn cover(&self, max_px: u32) -> Option<Rgba8Image>;
    fn search(&self, q: &Query) -> SearchIter<'_>;
    // fixed formats
    fn page_count(&self) -> Option<usize>;
    fn render_page(&self, i: usize, scale: f32) -> Result<Rgba8Image>;
    // reflowable formats
    fn content(&self) -> Option<&FlowContent>;  // styled block tree for the layout engine
}
```

### 2.4 Reflow layout engine (EPUB/MOBI/AZW3/TXT)
This is the biggest engineering piece and what makes "Deep Customization" possible.
- **Input:** XHTML + a CSS subset → our own `FlowContent` tree (blocks, inlines, images, tables-lite, lists, ruby).
- **Text shaping & line breaking:** `parley` (Linebender) on `swash`/`fontique`: bidi, Arabic shaping, **Thai/Lao/Khmer dictionary word-breaking**, CJK, hyphenation (`hyphenation` crate, Knuth-Liang patterns).
- **Pagination:** fills page boxes per viewport and settings; keeps a **stable location** (spine index + character offset, EPUB CFI-compatible) so position survives font/size changes.
- **Rasterization:** each page painted to a texture (`vello_cpu` or `tiny-skia`), which the GPU then bends during the curl. Page textures are cached ±2 pages.
- **Publisher vs. user styles:** "Publisher defaults" toggle like Thorium.
- **Stretch goal:** evaluate `blitz` (Rust HTML/CSS engine) for better CSS fidelity on complex EPUBs.

### 2.5 Deep customization
- **Themes:** Light, Sepia, Dark, OLED Black, Paper-texture, plus a custom theme editor (bg, text, link, selection, page-edge colours). Follow system light/dark automatically.
- **Fonts:** bundled open fonts (no web fonts): Literata, Source Serif 4, Atkinson Hyperlegible, OpenDyslexic, Noto Sans/Serif **Thai**, Noto Naskh **Arabic**, Noto Sans **CJK** (optional download-free separate pack), plus any **system font** and user-imported TTF/OTF.
- **Typography:** size, weight, line height, letter & word spacing, paragraph spacing, first-line indent, justification, hyphenation, text alignment.
- **Layout:** margins (per side), max line width, columns (1/2/auto), header/footer (title, chapter, page x/y, time left, clock) on/off.
- **Images:** invert in dark mode (smart, skips photos), brightness/contrast, PDF crop-margins.
- **Profiles:** save/load presets per format (e.g. "Manga", "Novel – night").

### 2.6 Reading direction & display
- **LTR / RTL** per book; auto-detected from EPUB `page-progression-direction`, `ComicInfo.xml` `Manga=YesAndRightToLeft`, or language (ar/he/fa/ur). RTL mirrors click zones, swipe direction, curl direction and spread order.
- **Display modes:** Single page · Two-page spread · **Auto** (spread when window aspect > 1.2). Plus continuous vertical scroll for PDF/webtoon.
- **First blank page control:** toolbar toggle in the top bar inserts/removes a virtual flyleaf so spreads pair correctly (e.g. cover alone on the right, then 2|3). Remembered per book.
- Zoom: fit page / fit width / custom; pan with drag when zoomed.

### 2.7 Interactive 3D bookshelf (library)
- Real-time 3D scene (wgpu): wooden shelves with instanced book meshes; spine shows title/author (rendered text texture); covers from a small thumbnail.
- **Shelf orientation:** spines-out (classic), covers-out (display), stacked/horizontal piles, and a flat 2D grid / list fallback (low-power & accessibility).
- Interactions: orbit/hover tilt, pull book off shelf → opens with a "book opening" transition; drag books between shelves; custom shelves/collections, sort by title/author/recent/progress; progress shown as a bookmark ribbon height.
- Import: file picker, drag-and-drop files/folders, "watch folder" (local only).

### 2.8 Thorium-parity essentials
- TOC panel, page list, go-to page/percentage.
- Bookmarks, highlights (4 colours), notes; export annotations as Markdown/JSON (local file only).
- Full-text search with result list and highlighting.
- Read Aloud (TTS) via **OS speech engines only** (SAPI/WinRT on Windows, AVSpeechSynthesizer on macOS, speech-dispatcher on Linux, Web Speech API on web — using only local voices where the browser exposes `localService`).
- Accessibility: keyboard-only operation, screen-reader exposure via `AccessKit` (egui supports it), high-contrast theme, reduced motion (disables curl & shelf animation).

---

## 3. Privacy & transparency design

### 3.1 Principles
1. **No network code in the binary.** Enforced in CI, not just promised.
2. **Books are referenced, not copied.** The library stores the file *path*; the book stays where the user put it. (Optional "Copy into library folder" for users who want it.)
3. **Store only what's needed to work well** (table below). Nothing about reading habits beyond position.
4. **Human-readable storage.** Settings and library are plain JSON/TOML the user can open.
5. **One-click forget.** Per book ("Forget this book": removes entry, position, annotations, cover thumb) and "Wipe everything".

### 3.2 What is stored (complete list)
| Data | Why | Where | Can delete |
|---|---|---|---|
| Book entry: id (BLAKE3 hash of size + first/last 64 KB), path, title, author, format, added date | Show library, find file again if renamed/moved | `library.json` | Per book / wipe |
| Last position (location string) + "first blank page" + direction overrides | Resume reading | `library.json` | Per book |
| Bookmarks, highlights, notes | User-created | `annotations/<id>.json` | Per book |
| Cover thumbnail (≤ 300 px WebP, optional) | 3D shelf | `covers/<id>.webp` | Toggle off "Save covers"; per book |
| Page render / layout cache | Speed | **RAM only** by default; optional disk cache with a size cap, auto-purged | Clear cache button |
| Settings & themes | Preferences | `settings.toml` | Reset |

**Not stored:** book text, full-size pages, reading time stats (opt-in local-only stats can come later), search history, anything about the device.

**Location:** OS app-data dir (`directories` crate) or **Portable mode** (`portable.flag` beside the exe → everything stored next to the exe, e.g. on a USB stick). On web: IndexedDB for the JSON, **OPFS** for book bytes if the user chooses to keep them in the browser; otherwise the File System Access API handle only.

### 3.3 Enforcement
- **`cargo-deny`** bans network crates (`reqwest`, `hyper`, `ureq`, `curl`, `isahc`, `tokio` with `net`, `async-std`, `surf`, `rustls` used for clients, `openssl`, `sentry`, …). CI fails if any appear in the tree.
- **CI egress test:** run the app headless through an import → read → curl → close scenario under a network sandbox (Linux `unshare -n` / Windows firewall rule in CI) and assert zero connection attempts (also `strace`-check for `connect()` on Linux).
- **PDFium built/configured without V8/XFA**; PDF links/URIs are never auto-followed. External links in books open in the system browser **only after a confirm dialog**.
- **Web build:** strict CSP `default-src 'self'; connect-src 'none'; img-src 'self' blob: data:; font-src 'self'`; service worker caches the app shell for offline use; no analytics; self-hosted only (no CDN).
- **"Privacy" screen in-app:** lists the storage folder, file sizes, a live view of `library.json`, and the buttons above. Plus `PRIVACY.md` in the repo and a reproducible-build note.
- **No auto-update pinging.** Updates via Microsoft Store/winget/Homebrew/Flatpak (the store does it, not us) or manual download.

---

## 4. Technology choices

| Concern | Choice | Why |
|---|---|---|
| Language | Rust (stable, 2024 edition) | Requirement; small, fast native binary |
| App shell / UI chrome | **`eframe` + `egui`** (wgpu backend) | Pure Rust, same code native + WASM, AccessKit accessibility, fast to iterate |
| GPU | **`wgpu`** custom render passes via egui paint callbacks | Page-curl mesh shader, 3D shelf, runs on DX12/Vulkan/Metal/WebGPU/WebGL2 |
| Windowing/events | `winit` (through eframe) | `CursorLeft/Entered`, touch, IME |
| Text layout | `parley` + `swash` + `fontique` | Shaping, bidi, Thai/CJK line breaking |
| 2D raster for pages | `vello_cpu` or `tiny-skia` | Deterministic CPU raster → texture |
| PDF | `pdfium-render` (+ bundled pdfium.dll/.dylib/.so; pdfium WASM on web) | Best-in-class fidelity; Chromium's engine; permissive licence |
| PDF fallback | `hayro` | Pure Rust, no native lib, small WASM |
| Archives/XML | `zip`, `quick-xml`, `roxmltree` | EPUB/CBZ/KF8 |
| Images | `image`, `zune-jpeg`, `webp`, `resvg` (SVG in EPUB) | |
| Encodings | `encoding_rs`, `chardetng` | TXT & legacy MOBI |
| Audio | `rodio` (desktop) / Web Audio via `web-sys` (web) | Paper sounds |
| Global hotkey / tray | `global-hotkey`, `tray-icon` | Boss key, minimize-to-tray |
| File dialogs | `rfd` (native + web) | |
| Storage | `serde` + `serde_json`/`toml`; web: `rexie` (IndexedDB) + OPFS | Human-readable |
| Hashing | `blake3` | Book identity |
| i18n | `fluent` | EN + TH first, then others |
| Packaging | `cargo-packager` (MSI/NSIS, .dmg/.app, AppImage/.deb), `trunk` for web | |

**Why not Tauri / Electron?** Thorium's Electron base is what we're beating on size and speed. Tauri would push the UI and the curl into JavaScript/WebView; keeping everything in Rust + wgpu gives one renderer on every platform and makes the zero-network guarantee auditable in one dependency tree.

**Why not MuPDF?** Excellent, but AGPL. PDFium (BSD-3) keeps licensing options open (MIT/Apache app, or a paid edition later).

---

## 5. Architecture

```
ghostleaf/                     (cargo workspace)
├─ app/                        binary: eframe app, desktop + wasm entry points
├─ crates/
│  ├─ gl-core/                 domain model: Book, Location, Settings, Library, events
│  ├─ gl-formats/              Document trait + pdf, epub, mobi, kf8, cbz, txt
│  ├─ gl-layout/               reflow engine (FlowContent → paginated pages)
│  ├─ gl-render/               wgpu: page textures, curl mesh/shader, shelf scene, veil
│  ├─ gl-anim/                 springs, gesture → curl state machine
│  ├─ gl-audio/                sound bank + mute
│  ├─ gl-store/                Storage trait: FsStore (native), WebStore (IndexedDB/OPFS)
│  ├─ gl-platform/             stealth hooks, hotkeys, tray, opacity, TTS bridges
│  └─ gl-ui/                   egui panels: top bar, TOC, settings, privacy screen
├─ assets/                     fonts, sounds (CC0), shelf textures, icons
├─ xtask/                      fetch pdfium binaries, bundle, license report
├─ deny.toml                   network-crate ban list
└─ docs/  PRIVACY.md  ARCHITECTURE.md
```

**Threads:** UI thread (egui + wgpu) · worker pool for parsing, layout, PDF raster (on web: `wasm-bindgen-rayon` or a single Web Worker) · audio thread. Messages over channels; the UI never blocks on I/O.

**Page pipeline:** `Document` → (reflow: `gl-layout` paginate) → page raster at device scale → GPU texture (cache N±2) → curl/slide pass → screen.

### 5.1 Page-curl rendering (summary)
- Page = grid mesh (~48×64 vertices). Vertex shader applies **cylinder deformation**: given a fold line (point + direction from the drag) and radius *r*, vertices past the line wrap around the cylinder; past half circumference they lie flat on the back side.
- Fragment shader: front face samples page *n*; back face samples page *n* mirrored and tinted (paper translucency setting) ; ambient-occlusion-like gradient near the fold; drop shadow on page *n+1* from the curled area.
- Corner grabs give a diagonal fold; edge grabs a vertical fold; fold radius shrinks as the page approaches completion for a natural snap.
- Same shader handles RTL by mirroring the fold space; spread mode renders two page quads with the curl on the active one.
- **v1 implementation:** the cylinder deformation is computed on the CPU (~3k vertices per frame, negligible) and drawn as a textured `egui::Mesh`, with shading baked into vertex colours. This runs unchanged on every egui backend (wgpu, glow, web, mobile). A dedicated WGSL shader stays an optimization for later.

### 5.2 Stealth state machine
`Visible → (CursorLeft | FocusLost | BossKey) → Hidden(style) → (CursorEntered + delay | click | BossKey) → Visible`, with a `Suspended` state during drags and dialogs.

---

## 6. Platform matrix

| Platform | Build | Notes |
|---|---|---|
| Windows 10/11 x64 + ARM64 | MSVC toolchain; MSI + portable ZIP; Microsoft Store (MSIX) later | Primary target; DX12 backend; opacity/click-through via Win32 |
| macOS 12+ (Apple Silicon + Intel) | `.app`/`.dmg`, **unsigned** (no Apple Developer account) | Metal; first open needs right-click → Open; boss key needs Accessibility permission (prompted only if enabled) |
| Linux x64 | AppImage, `.deb`, Flatpak | Vulkan/GL; Wayland: opacity/always-on-top depend on compositor → Veil style default |
| Android 8+ (arm64-v8a) | `cargo-ndk` + Gradle → APK on GitHub Releases | In progress; same egui app as desktop; see §6.1 |
| iOS | Only if buildable without a paid Apple account; otherwise the web app | Deferred |
| Web (WASM) | `trunk build --release`; installable PWA; static hosting or open locally | WebGPU with WebGL2 fallback; files via File System Access API (Chromium) or `<input type=file>`; no global boss key (in-page key only) |

Targets: **binary ≤ 25 MB** (incl. PDFium ~5 MB, excl. optional CJK font pack), **cold start < 400 ms**, **page turn at a steady 60 fps on integrated GPUs**, **idle RAM < 150 MB** with a 500-page PDF open.

### 6.1 Mobile (Android now, iOS undecided)
**Approach (decided, Android):** the **same egui app** runs on Android through a small `NativeActivity` subclass (`MainActivity.java`) that handles the document picker, "Open with", system-bar colours and file descriptors over JNI. Everything else (formats, curl, sounds, settings, library) is shared with desktop. The manifest declares **no permissions**.

**iOS:** no paid Apple Developer account, so there's no App Store or TestFlight. A native build is attempted only if it can be installed without one; otherwise iPhone/iPad users get the web app (WASM PWA).

**Mobile-specific design**
- **Privacy you can verify:** the Android manifest has **no `INTERNET` permission**, so the OS itself blocks network access. iOS: App Store label "Data Not Collected"; the same `cargo-deny` gate keeps networking code out.
- **Import:** Android Storage Access Framework (persistable URI permission, no copy needed); iOS `UIDocumentPicker` + "Open in Ghostleaf". Books stay in place where the OS allows; otherwise they're copied into the app sandbox and shown on the Privacy screen.
- **Stealth without a mouse:** blur the app snapshot in the app switcher; optional Android `FLAG_SECURE` (blocks screenshots/recents preview); **panic veil** via two-finger tap or turning the phone face-down; unveil with a tap or biometric.
- **Reading UX:** single page in portrait, spread in landscape on tablets; finger-driven curl with velocity physics; optional haptic tick; bottom-sheet settings; volume-key page turning (Android).
- **PDF engine:** prebuilt PDFium for Android (arm64-v8a, armeabi-v7a, x86_64) and iOS (xcframework).
- **Builds & distribution:** Android via `cargo-ndk` + Gradle → signed APK on GitHub Releases. Google Play maybe later; F-Droid optional. CI on GitHub Actions.


---

## 7. Roadmap

### Done so far (2026-11-11)
- **Desktop core:** Windows + Linux builds of the v1 feature set: PDF/TXT/CSV, 3D curl (spine-safe, soft shadows, gutter), slide/none, recorded paper sounds + mute, stealth veil, single/spread/auto, LTR/RTL, first blank page, 8 themes + custom tint, title-bar options, library, bookmarks, Privacy screen, English + Thai.
- **CI/release:** CI on Windows, Linux and Android; `cargo-deny` network ban; tag `v*` → draft GitHub Release with Windows + Linux packages and SHA256SUMS; recorded sounds are an encrypted file in the repo, decrypted in CI with a secret.
- **Android phases 1–2:** APK builds in CI; touch stealth (hide on focus loss, tap to reveal); compact phone layout; status-bar insets and bar colours; Open… via the system picker (reads through the granted file descriptor, no storage permission); "Open with"; reader top bar with show/hide tab that never triggers a page turn.

### Step 1 — Desktop v0.1.0 release (small)
- Owner runs the Windows checklist: boss key, Minimize hide style, `.exe` icon, high-DPI, a large PDF (memory, smoothness), Thai UI.
- Fix what turns up, write release notes, tag `v0.1.0`.

### Step 2 — Android phase 3: polish (medium)
- Tablets/landscape: two-page spread in landscape; rotation keeps the current page.
- Edge-to-edge for target API 35 (content placed correctly under status/navigation bars).
- ✅ Volume keys turn pages (toggle), keep screen on while reading, optional haptic tick on a turn (built 2026-11-16, awaiting device test).
- ✅ Library and settings are saved the moment the app goes to the background, so the reading position survives Android closing it.
- Predictive back gesture (with the API 35 work).

### Step 3 — Android phase 4: mobile stealth (medium)
- Blurred/blank thumbnail in Recents.
- Optional screenshot/recording block (`FLAG_SECURE`).
- Panic veil: two-finger tap, or phone turned face-down (motion sensor, no permission).
- Reveal: tap, or optionally fingerprint/face (`USE_BIOMETRIC`, decided yes).

### Step 4 — Android phase 5: proper releases (small)
- Release signing key stored as an encrypted GitHub secret; signed APK attached to each GitHub Release next to Windows + Linux.
- versionCode/versionName derived from the tag.
- Google Play: maybe later (needs a $25 account + privacy policy). Not now.

### Step 5 — Markdown reader (small–medium)
- `.md` / `.markdown` in the library, picker, drag-and-drop and Android "Open with".
- Headings (TOC from headings), paragraphs, bold/italic/strikethrough, inline code and code blocks, block quotes, lists and task lists, horizontal rules, GFM tables, footnotes, local images.
- Privacy: remote images and links are never loaded automatically; raw HTML is not executed.
- First user of rich styled text, so it's groundwork for the EPUB engine in Step 7.

### Step 6 — v1.1: macOS + Linux packages (small–medium)
- Linux: AppImage + `.deb` (Flatpak later).
- macOS: unsigned `.dmg` (Apple Silicon + Intel); README explains the first-open warning.

### Step 7 — v1.2: EPUB + CBZ + deep customization (large)
Reflow engine (§2.4), EPUB 2/3 + fixed layout, comics with manga RTL detection, hyphenation, saved style presets, publisher-styles toggle. Reuses the TXT/CSV text pipeline.

### Step 8 — v1.3: MOBI / AZW / AZW3 (medium)
PalmDOC/HUFF, KF8; DRM-protected files rejected with a clear message.

### Step 9 — v1.4: 3D bookshelf + Thorium parity (large)
3D shelves (§2.7), highlights/notes, full-text search, read-aloud with system voices, accessibility pass.

### Later
- **Web app (WASM PWA):** also the iPhone/iPad route if a native iOS build isn't possible without a paid Apple account.
- iOS native build: only if installable without a paid account.
- CBR, FB2, DjVu.

## 8. Testing & quality
- **Corpus:** public-domain books (Project Gutenberg EPUB/TXT, Standard Ebooks), PDF.js & PDFBox test PDFs, synthetic MOBI/AZW3 via Calibre conversion, Thai/Arabic/Japanese samples, manga CBZ.
- **Golden-image tests** for layout and PDF raster (per-pixel tolerance).
- **Fuzzing** every parser (malicious files are the main security risk for an offline app).
- **Property tests** for location stability across font/size changes.
- **Perf CI:** frame-time histogram during scripted curls; startup time; memory.
- **Privacy CI:** `cargo-deny` + sandboxed egress test (see §3.3).

---

## 9. Risks & mitigations
| Risk | Impact | Mitigation |
|---|---|---|
| EPUB CSS fidelity (custom layout engine) | High | Limited CSS subset + "publisher styles" toggle; evaluate `blitz` later; prioritize real-world corpus |
| PDFium native binary distribution per OS/arch & WASM size (~4–5 MB) | Medium | `xtask` fetches pinned pdfium-binaries; `hayro` fallback; lazy-load PDF engine on web |
| Curl looks "cheap" | High (it's the signature feature) | Prototype in M0/M3 early; tune with reference videos; paper translucency, shadow, fold-radius curves |
| Stealth on Wayland/macOS restrictions | Medium | Veil works everywhere; advanced modes per-platform |
| DRM'd AZW user confusion | Low | Clear message + docs |
| Kindle format quirks (KF8 edge cases) | Medium | Fuzz + corpus; graceful fallback to text-only |
| Font licensing | Low | Only OFL/Apache fonts bundled |

---

## 10. Licensing & distribution
- **Decided: MIT OR Apache-2.0** for the app (deps are compatible: PDFium BSD-3, fonts OFL). Paper sounds come from the Sonniss GDC bundle (royalty-free; raw files aren't redistributed, so the repo holds only an encrypted copy, see docs/SOUNDS.md).
- Distribution now: **GitHub Releases only** (Windows, Linux, Android APK; macOS unsigned from v1.1). Later options: Google Play, winget, Flathub, F-Droid, static web build. No paid Apple distribution.
- Optional monetization that keeps privacy: one-time "supporter" purchase via the store; no accounts.

---

## 11. Name & logo
**Name: Ghostleaf.** Also considered: Quire, Dogear, Hushpage, Ferrofolio.

**Logo: A · Leaf-fold** (decided). A geometric leaf built from two quarter-circle arcs, its tip turned into a folded page corner: leaf + page + turning. Monochrome: ink `#121212`, paper `#F4F1EA`. Files: `branding/a-leaf-fold.svg` (mark), `a-leaf-fold-icon-dark.svg` / `-light.svg` (app icons), and `app/assets/icons/ghostleaf.ico`.

**Wordmark:** lowercase "ghostleaf" set in **MuseoModerno Italic** (Medium). Used only for the app name and logo lockups, never for body or reading text.

Alternatives B (Open leaves) and C (Vanish) are kept in `branding/` for reference.

## 12. v1 implementation notes
- Workspace: `crates/gl-core` (settings, library, JSON store), `crates/gl-formats` (PDF via PDFium, TXT and CSV via cosmic-text, Thai word breaks via ICU4X), `crates/gl-curl` (curl geometry + springs, unit-tested), `app` (eframe/egui 0.37).
- The page curl is a CPU-deformed 36×48 grid drawn as textured egui meshes (front, back, shadows). It runs on any egui backend, so mobile can reuse it unchanged.
- Each open book has its own worker thread (layout + rendering); the UI thread never blocks.
- Paper sounds: recorded (Sonniss GDC) when the decrypted files are present at build time, otherwise synthesized (filtered noise and flutter).
- Verified on Linux (software GL): library, PDF/TXT/CSV, spread and single, curl forward/back by drag and click, RTL + flyleaf, dark theme, stealth veil, settings, privacy, Thai UI. **Not yet run on Windows.**

### 2026-11-11 — first Windows feedback (fixed)
- Settings window: fixed width and a proper scroll area.
- Curl: the fold may never cross the spine (the bound edge stays flat), and shading is clipped to the turning sheet, so a curl can no longer bend both pages. Pages sit below the auto-hiding top bar so top corners can be grabbed.
- Title bar: *Match theme* (Windows 11 DWM caption colours, default), *System*, or *Hidden* (frameless, with Ghostleaf's own minimize/maximize/close, drag-to-move and edge resize).
- Themes: Light, Paper, Sepia, Tan, Dusk, Dark, Black, and Custom (any tint, 0–100% strength; ink and UI colours are derived for contrast).
- Sounds: recorded page turns from the Sonniss GDC bundle (flip / peel / land), embedded at build time if present (git-ignored), with synthesized fallback. See docs/SOUNDS.md.

## 13. Still open
1. Web app: hosted publicly or offline-only bundle; also decides the iPhone/iPad route.
2. Windows checks before v0.1.0: boss key, minimize-style stealth, `.exe` icon embedding, high-DPI.
3. Google Play / F-Droid: later, after signed APKs are on GitHub Releases.
