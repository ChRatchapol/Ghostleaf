//! Ghostleaf — a private, offline reader for PDF, TXT and CSV.
//!
//! Privacy: this program contains no networking code. It reads the files you
//! open, and stores only settings, the library list (paths + reading position)
//! and optional small cover thumbnails in one local folder. See the Privacy
//! screen in the app, or PRIVACY.md.
//!
//! This crate is shared by the desktop executable (`src/main.rs`) and the
//! Android library (`crates/gl-android`).

#[cfg(target_os = "android")]
mod android;
mod app;
mod curl_draw;
mod i18n;
mod icons;
mod library;
mod panels;
mod reader;
mod sound;
mod stealth;
mod style;
mod titlebar;
mod worker;

use std::path::PathBuf;

/// System bar insets in physical pixels: [top, bottom, left, right].
pub type InsetsFn = Box<dyn Fn() -> [f32; 4]>;

/// Where a run of the app starts.
#[derive(Default)]
pub struct Launch {
    /// Areas covered by system bars (Android status / navigation bar).
    pub insets: Option<InsetsFn>,
    /// A file to open straight away (desktop "Open with…").
    pub open_file: Option<PathBuf>,
    /// A folder whose books are added to the library at start-up (Android phase 1:
    /// the app's own `Books` folder, reachable over USB).
    pub inbox: Option<PathBuf>,
}

fn init_text() {
    gl_formats::engine::register_bundled_fonts(style::reading_fonts());
    if i18n::system_is_thai() {
        gl_formats::decode::set_region_hint("th");
    }
    // Load fonts for the page renderer in the background (can take ~0.5 s).
    std::thread::spawn(|| drop(gl_formats::engine::engine()));
}

/// Run with the given native options (window, renderer, Android app handle…).
pub fn run(options: eframe::NativeOptions, launch: Launch) -> eframe::Result {
    init_text();
    eframe::run_native(
        "Ghostleaf",
        options,
        Box::new(move |cc| Ok(Box::new(app::GhostleafApp::new(cc, launch)))),
    )
}

/// The path to read a library entry from: on Android, books picked through the
/// system picker are stored as `content://` URIs and opened through a file
/// descriptor; everywhere else the path is used as is.
pub(crate) fn resolve_book_path(path: &std::path::Path) -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    if let Some(uri) = path.to_str().filter(|s| s.starts_with("content://")) {
        return android::open_uri(uri);
    }
    Some(path.to_path_buf())
}

/// Whether a library entry can (probably) still be opened.
pub(crate) fn book_available(path: &std::path::Path) -> bool {
    path.to_str().is_some_and(|s| s.starts_with("content://")) || path.exists()
}

/// Release whatever [`resolve_book_path`] opened (Android file descriptors).
pub(crate) fn release_book_path(_path: &std::path::Path) {
    #[cfg(target_os = "android")]
    android::close_path(_path);
}

/// Desktop entry point.
#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result {
    use std::sync::Arc;
    let open_file = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.is_file());
    let renderer = match std::env::var("GHOSTLEAF_RENDERER").as_deref() {
        Ok("glow" | "gl" | "opengl") => eframe::Renderer::Glow,
        _ => eframe::Renderer::Wgpu,
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Ghostleaf")
        .with_app_id("ghostleaf")
        .with_inner_size([1180.0, 800.0])
        .with_min_inner_size([520.0, 380.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/icon-256.png")) {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer,
        ..Default::default()
    };
    run(
        options,
        Launch {
            open_file,
            inbox: None,
            insets: None,
        },
    )
}

/// Android entry point, called from `android_main` in `crates/gl-android`.
#[cfg(target_os = "android")]
pub fn run_android(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("ghostleaf"),
    );
    std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));
    // App-private storage; no permissions needed, removed on uninstall.
    if let Some(dir) = app.internal_data_path() {
        gl_core::set_platform_root(dir.join("ghostleaf"));
    }
    // Android/data/app.ghostleaf/files/Books: copy books here over USB (phase 1).
    let inbox = app.external_data_path().map(|d| d.join("Books"));
    if let Some(d) = &inbox {
        let _ = std::fs::create_dir_all(d);
    }
    // JNI bridge to MainActivity (file picker, "Open with", system bar colours)
    android::init(app.vm_as_ptr(), app.activity_as_ptr());
    let a2 = app.clone();
    let insets: InsetsFn = Box::new(move || {
        // content_rect = the part of the window not covered by system bars (or the keyboard)
        let r = a2.content_rect();
        if r.right <= r.left || r.bottom <= r.top {
            return [0.0; 4];
        }
        let (w, h) = a2.native_window().map(|w| (w.width(), w.height())).unwrap_or((r.right, r.bottom));
        [
            r.top.max(0) as f32,
            (h - r.bottom).max(0) as f32,
            r.left.max(0) as f32,
            (w - r.right).max(0) as f32,
        ]
    });
    let options = eframe::NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if let Err(e) = run(
        options,
        Launch {
            open_file: None,
            inbox,
            insets: Some(insets),
        },
    ) {
        log::error!("Ghostleaf stopped: {e}");
    }
}
