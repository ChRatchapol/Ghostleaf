//! Ghostleaf — a private, offline reader for PDF, TXT and CSV.
//!
//! Privacy: this program contains no networking code. It reads the files you
//! open, and stores only settings, the library list (paths + reading position)
//! and optional small cover thumbnails in one local folder. See the Privacy
//! screen in the app, or PRIVACY.md.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

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

use std::sync::Arc;

fn main() -> eframe::Result {
    gl_formats::engine::register_bundled_fonts(style::reading_fonts());
    if i18n::system_is_thai() {
        gl_formats::decode::set_region_hint("th");
    }
    // Load system fonts for the page renderer in the background (can take ~0.5 s).
    std::thread::spawn(|| drop(gl_formats::engine::engine()));

    let startup_file = std::env::args_os().nth(1).map(std::path::PathBuf::from).filter(|p| p.is_file());

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
    eframe::run_native(
        "Ghostleaf",
        options,
        Box::new(move |cc| Ok(Box::new(app::GhostleafApp::new(cc, startup_file)))),
    )
}
