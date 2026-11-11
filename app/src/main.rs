//! Desktop executable. All of the app lives in the `ghostleaf_app` library.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    ghostleaf_app::run_desktop()
}
