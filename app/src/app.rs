//! Top-level app state: library ↔ reader, windows, stealth, persistence.

use crate::i18n::{self, Tr};
use crate::library::{Covers, LibAction, library_ui};
use crate::panels::{self, PrivacyAction};
use crate::reader::{Action, Env, Reader};
use crate::sound::Sound;
use crate::stealth::{self, Stealth};
use crate::style;
use gl_core::{Format, Library, Settings, Store, TitleBar, book_id};
use std::path::{Path, PathBuf};

/// Windows `canonicalize` returns `\\?\C:\...`; keep the ordinary form for display and storage.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}

pub struct GhostleafApp {
    store: Store,
    settings: Settings,
    library: Library,
    reader: Option<Reader>,
    sound: Sound,
    stealth: Stealth,
    covers: Covers,
    show_settings: bool,
    show_privacy: bool,
    wipe_confirm: bool,
    settings_dirty: bool,
    library_dirty: bool,
    last_save: f64,
    applied_theme: Option<u64>,
    applied_titlebar: Option<(TitleBar, u64)>,
    applied_on_top: Option<bool>,
    title_set: String,
    font_list: Option<Vec<String>>,
    toast: Option<(String, f64)>,
    startup_file: Option<PathBuf>,
    inbox: Option<PathBuf>,
    insets: Option<crate::InsetsFn>,
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    was_focused: bool,
}

impl GhostleafApp {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: crate::Launch) -> Self {
        let crate::Launch {
            open_file: startup_file,
            inbox,
            insets,
        } = launch;
        style::install_fonts(&cc.egui_ctx);
        let store = Store::open();
        let settings = store.load_settings();
        let library = store.load_library();
        Self {
            store,
            settings,
            library,
            reader: None,
            sound: Sound::new(),
            stealth: Stealth::new(),
            covers: Covers::new(),
            show_settings: false,
            show_privacy: false,
            wipe_confirm: false,
            settings_dirty: false,
            library_dirty: false,
            last_save: 0.0,
            applied_theme: None,
            applied_titlebar: None,
            applied_on_top: None,
            title_set: String::new(),
            font_list: None,
            toast: None,
            startup_file,
            inbox,
            insets,
            was_focused: true,
        }
    }

    fn t(&self) -> &'static Tr {
        i18n::tr(self.settings.language)
    }

    /// Add a file to the library (no copy) and return its id.
    fn import(&mut self, path: &Path) -> Option<String> {
        let path = path.canonicalize().map(strip_verbatim).unwrap_or_else(|_| path.to_path_buf());
        let format = Format::from_path(&path).or_else(|| {
            let mut b = [0u8; 5];
            use std::io::Read;
            std::fs::File::open(&path).ok()?.read_exact(&mut b).ok()?;
            (&b == b"%PDF-").then_some(Format::Pdf)
        })?;
        let id = book_id(&path).ok()?;
        let title = gl_formats::title_from_path(&path);
        self.library.upsert(id.clone(), path, title, format);
        self.library_dirty = true;
        Some(id)
    }

    fn open_book(&mut self, id: &str, ctx: &egui::Context) {
        if let Some(b) = self.library.get_mut(id) {
            b.last_opened = gl_core::now_secs();
            let mut b = b.clone();
            match crate::resolve_book_path(&b.path) {
                Some(p) => b.path = p,
                None => {
                    let msg = i18n::tr(self.settings.language).file_missing.to_string();
                    self.toast = Some((msg, ctx.input(|i| i.time) + 3.0));
                    return;
                }
            }
            self.reader = Some(Reader::open(&b, ctx, &self.settings));
            self.library_dirty = true;
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_os = "android"))]
        let files = rfd::FileDialog::new()
            .add_filter("PDF, TXT, CSV", gl_core::Format::EXTENSIONS)
            .add_filter("PDF", &["pdf"])
            .add_filter("Text", &["txt", "text", "md", "log"])
            .add_filter("CSV / TSV", &["csv", "tsv", "tab"])
            .pick_files();
        // Android: the system picker runs in MainActivity; results are collected
        // when the app regains focus (see `poll_android_documents`).
        #[cfg(target_os = "android")]
        let files: Option<Vec<PathBuf>> = {
            crate::android::pick_documents();
            None
        };
        let now = ctx.input(|i| i.time);
        self.stealth.suppress(now, 1.0);
        let Some(files) = files else { return };
        let mut first = None;
        for f in files {
            if let Some(id) = self.import(&f) {
                first.get_or_insert(id);
            }
        }
        if let Some(id) = first {
            self.open_book(&id, ctx);
        }
    }

    fn save(&mut self, now: f64, force: bool) {
        if !force && now - self.last_save < 1.0 {
            return;
        }
        if self.settings_dirty {
            let _ = self.store.save_settings(&self.settings);
            self.settings_dirty = false;
        }
        if self.library_dirty {
            let _ = self.store.save_library(&self.library);
            self.library_dirty = false;
        }
        self.last_save = now;
    }

    fn sync_window(&mut self, ctx: &egui::Context, frame: &eframe::Frame, hidden: bool) {
        let pal = style::palette(&self.settings);
        let key = style::palette_key(&pal);
        if self.applied_theme != Some(key) {
            style::apply(ctx, pal);
            self.applied_theme = Some(key);
        }
        let tb = (self.settings.title_bar, key);
        #[cfg(target_os = "android")]
        if self.applied_titlebar != Some(tb) {
            crate::android::set_bar_colors([pal.bg.r(), pal.bg.g(), pal.bg.b()], !pal.dark);
            self.applied_titlebar = Some(tb);
        }
        if !cfg!(target_os = "android") && self.applied_titlebar != Some(tb) {
            let was_hidden = matches!(self.applied_titlebar, Some((TitleBar::Hidden, _)));
            match self.settings.title_bar {
                TitleBar::Hidden => ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(false)),
                TitleBar::Theme => {
                    if was_hidden {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                    }
                    crate::titlebar::set_native_colors(frame, Some((pal.bg, pal.ink, pal.dark)));
                }
                TitleBar::System => {
                    if was_hidden {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
                    }
                    crate::titlebar::set_native_colors(frame, None);
                }
            }
            self.applied_titlebar = Some(tb);
        }
        if self.settings.title_bar == TitleBar::Hidden {
            crate::titlebar::resize_edges(ctx);
        }
        if self.applied_on_top != Some(self.settings.always_on_top) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.settings.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
            self.applied_on_top = Some(self.settings.always_on_top);
        }
        self.stealth.sync_boss_key(self.settings.stealth.boss_key, ctx);
        let title = if hidden {
            "Document".to_string()
        } else if self.settings.stealth.neutral_title {
            "Ghostleaf".to_string()
        } else {
            match &self.reader {
                Some(r) => format!("{} — Ghostleaf", r.title),
                None => "Ghostleaf".to_string(),
            }
        };
        if title != self.title_set {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_set = title;
        }
    }
}

impl eframe::App for GhostleafApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Keep the UI clear of system bars (Android status / navigation bar).
        let [top, bottom, left, right] = self.insets.as_ref().map(|f| f()).unwrap_or([0.0; 4]);
        if top + bottom + left + right > 0.0 {
            let ppp = ui.ctx().pixels_per_point();
            let full = ui.max_rect();
            ui.painter().rect_filled(full, 0.0, style::palette(&self.settings).bg);
            let inner = egui::Rect::from_min_max(
                full.min + egui::vec2(left / ppp, top / ppp),
                full.max - egui::vec2(right / ppp, bottom / ppp),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.ui_inner(ui, frame));
        } else {
            self.ui_inner(ui, frame);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save(f64::MAX, true);
    }
}

impl GhostleafApp {
    /// Android: collect documents from the system picker / "Open with". They are
    /// queued in MainActivity, so we look when the app (re)gains focus and
    /// shortly after start-up (an "Open with" launch).
    #[cfg(target_os = "android")]
    fn poll_android_documents(&mut self, ctx: &egui::Context, now: f64) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        let regained = focused && !self.was_focused;
        self.was_focused = focused;
        if now < 3.0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        } else if !regained {
            return;
        }
        let mut first = None;
        for d in crate::android::take_documents() {
            if let Some(id) = self.import_uri(&d.uri, &d.name) {
                first.get_or_insert(id);
            }
        }
        if let Some(id) = first {
            self.open_book(&id, ctx);
        }
    }

    /// Add a picked document (content URI) to the library. Nothing is copied:
    /// we read it once through a descriptor to identify it, and keep the URI.
    #[cfg(target_os = "android")]
    fn import_uri(&mut self, uri: &str, name: &str) -> Option<String> {
        let path = crate::android::open_uri(uri)?;
        let format = Format::from_path(Path::new(name)).or_else(|| {
            let mut b = [0u8; 5];
            use std::io::Read;
            std::fs::File::open(&path).ok()?.read_exact(&mut b).ok()?;
            (&b == b"%PDF-").then_some(Format::Pdf)
        });
        let id = book_id(&path).ok();
        crate::android::close_path(&path);
        let (format, id) = (format?, id?);
        let title = gl_formats::title_from_path(Path::new(name));
        self.library.upsert(id.clone(), PathBuf::from(uri), title, format);
        self.library_dirty = true;
        Some(id)
    }

    fn ui_inner(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let t = self.t();
        let pal = style::palette(&self.settings);
        let frameless = self.settings.title_bar == TitleBar::Hidden;

        if let Some(dir) = self.inbox.take() {
            // Add every supported book in the inbox folder (no copy, no auto-open).
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_file() && Format::from_path(&p).is_some() {
                        self.import(&p);
                    }
                }
            }
        }
        if let Some(f) = self.startup_file.take()
            && let Some(id) = self.import(&f)
        {
            self.open_book(&id, &ctx);
        }

        #[cfg(target_os = "android")]
        self.poll_android_documents(&ctx, now);

        // drag & drop
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            let mut first = None;
            for p in dropped {
                if let Some(id) = self.import(&p) {
                    first.get_or_insert(id);
                }
            }
            if let Some(id) = first {
                self.open_book(&id, &ctx);
            }
        }

        let hidden = self.stealth.update(&ctx, &self.settings.stealth);
        self.sync_window(&ctx, frame, hidden);

        // Back button (Android) / Escape: close an open window first.
        if (self.show_settings || self.show_privacy)
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack) || i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            })
        {
            self.show_settings = false;
            self.show_privacy = false;
        }

        // screens
        let mut close_reader = false;
        if let Some(reader) = &mut self.reader {
            if let Some(book) = self.library.get_mut(&reader.book_id) {
                let mut env = Env {
                    settings: &mut self.settings,
                    book,
                    sound: &mut self.sound,
                    t,
                    pal,
                    settings_dirty: &mut self.settings_dirty,
                    library_dirty: &mut self.library_dirty,
                    frameless,
                };
                match reader.ui(ui, &mut env) {
                    Action::Close => close_reader = true,
                    Action::OpenSettings => self.show_settings = true,
                    Action::None => {}
                }
                if let Some(img) = reader.cover.take()
                    && self.settings.save_covers
                {
                    let small = img.downscale_to_width(240);
                    let mut png = Vec::new();
                    let enc = image::codecs::png::PngEncoder::new(&mut png);
                    use image::ImageEncoder;
                    if enc
                        .write_image(&small.pixels, small.width, small.height, image::ExtendedColorType::Rgba8)
                        .is_ok()
                    {
                        let _ = self.store.save_cover(&reader.book_id, &png);
                        self.covers.invalidate(&reader.book_id);
                    }
                }
            } else {
                close_reader = true;
            }
        } else {
            match library_ui(ui, &self.library, &self.store, &mut self.covers, t, pal, frameless) {
                LibAction::Open(id) => self.open_book(&id, &ctx),
                LibAction::OpenDialog => self.open_dialog(&ctx),
                LibAction::Settings => self.show_settings = true,
                LibAction::Privacy => self.show_privacy = true,
                LibAction::Remove(id) => {
                    self.library.remove(&id);
                    self.store.delete_cover(&id);
                    self.covers.invalidate(&id);
                    self.library_dirty = true;
                }
                LibAction::None => {}
            }
        }
        if close_reader {
            if let Some(r) = &self.reader {
                crate::release_book_path(&r.path);
            }
            self.reader = None;
            self.library_dirty = true;
        }

        // windows
        if self.show_settings && panels::settings_window(&ctx, &mut self.show_settings, &mut self.settings, t, &mut self.font_list) {
            self.settings_dirty = true;
        }
        if self.show_privacy {
            match panels::privacy_window(
                &ctx,
                &mut self.show_privacy,
                &self.store,
                &mut self.settings,
                t,
                pal,
                &mut self.wipe_confirm,
            ) {
                PrivacyAction::ClearCovers => {
                    self.store.clear_covers();
                    self.covers.clear();
                }
                PrivacyAction::ForgetBooks => {
                    self.reader = None;
                    self.library = Library::default();
                    self.store.clear_covers();
                    self.covers.clear();
                    self.library_dirty = true;
                }
                PrivacyAction::WipeAll => {
                    self.reader = None;
                    let _ = self.store.wipe_all();
                    self.library = Library::default();
                    self.settings = Settings::default();
                    self.covers.clear();
                    self.applied_theme = None;
                    self.settings_dirty = false;
                    self.library_dirty = false;
                }
                PrivacyAction::SettingsChanged => self.settings_dirty = true,
                PrivacyAction::None => {}
            }
        }

        if let Some((msg, until)) = &self.toast {
            if now < *until {
                egui::Area::new(egui::Id::unique("toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
                    .order(egui::Order::Foreground)
                    .show(&ctx, |ui| {
                        egui::Frame::new()
                            .fill(pal.ink)
                            .corner_radius(16)
                            .inner_margin(egui::Margin::symmetric(14, 8))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(msg).color(pal.paper));
                            });
                    });
            } else {
                self.toast = None;
            }
        }

        // stealth cover goes on top of everything
        if hidden {
            stealth::paint_cover(&ctx, self.settings.stealth.style, pal, t.reveal_hint);
        }

        self.save(now, false);
        if self.settings_dirty || self.library_dirty {
            ctx.request_repaint_after(std::time::Duration::from_millis(1100));
        }
    }
}
