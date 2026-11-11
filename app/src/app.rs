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
}

impl GhostleafApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup_file: Option<PathBuf>) -> Self {
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
            let b = b.clone();
            self.reader = Some(Reader::open(&b, ctx, &self.settings));
            self.library_dirty = true;
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let files = rfd::FileDialog::new()
            .add_filter("PDF, TXT, CSV", gl_core::Format::EXTENSIONS)
            .add_filter("PDF", &["pdf"])
            .add_filter("Text", &["txt", "text", "md", "log"])
            .add_filter("CSV / TSV", &["csv", "tsv", "tab"])
            .pick_files();
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
        if self.applied_titlebar != Some(tb) {
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
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let t = self.t();
        let pal = style::palette(&self.settings);
        let frameless = self.settings.title_bar == TitleBar::Hidden;

        if let Some(f) = self.startup_file.take()
            && let Some(id) = self.import(&f)
        {
            self.open_book(&id, &ctx);
        }

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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save(f64::MAX, true);
    }
}
