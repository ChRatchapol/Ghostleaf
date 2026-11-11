//! Window chrome: colour the OS title bar to match the theme (Windows 11),
//! or hide it and draw minimal window controls ourselves.

use crate::style::Palette;
use egui::{Color32, CursorIcon, Rect, ResizeDirection, Sense, Ui, ViewportCommand, vec2};

/// Colour the native caption (Windows 11 DWM). `None` restores the system default.
/// No-op on other platforms and on Windows 10 (the attributes are ignored there).
#[cfg(windows)]
pub fn set_native_colors(frame: &eframe::Frame, colors: Option<(Color32, Color32, bool)>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
    };
    let Ok(handle) = frame.window_handle() else { return };
    let RawWindowHandle::Win32(w) = handle.as_raw() else { return };
    let hwnd = w.hwnd.get() as *mut core::ffi::c_void;
    const DEFAULT: u32 = 0xFFFF_FFFF; // DWMWA_COLOR_DEFAULT
    let colorref = |c: Color32| -> u32 { c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16 };
    let (caption, text, dark) = match colors {
        Some((bg, fg, dark)) => (colorref(bg), colorref(fg), dark as i32),
        None => (DEFAULT, DEFAULT, 0),
    };
    let set = |attr: i32, value: *const core::ffi::c_void| {
        // SAFETY: hwnd is our live top-level window; value points to a 4-byte value that outlives the call.
        unsafe {
            DwmSetWindowAttribute(hwnd, attr as u32, value, 4);
        }
    };
    set(DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const i32 as *const _);
    set(DWMWA_CAPTION_COLOR, &caption as *const u32 as *const _);
    set(DWMWA_TEXT_COLOR, &text as *const u32 as *const _);
    set(DWMWA_BORDER_COLOR, &caption as *const u32 as *const _);
}

#[cfg(not(windows))]
pub fn set_native_colors(_frame: &eframe::Frame, _colors: Option<(Color32, Color32, bool)>) {}

#[derive(Clone, Copy)]
enum Ctl {
    Min,
    Max,
    Close,
}

/// Minimize / maximize / close, drawn as thin line icons. Use inside a right-to-left layout.
pub fn window_controls(ui: &mut Ui, pal: Palette) {
    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    for ctl in [Ctl::Close, Ctl::Max, Ctl::Min] {
        let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 30.0), Sense::click());
        let hovered = resp.hovered();
        let (fill, fg) = match (ctl, hovered) {
            (Ctl::Close, true) => (Color32::from_rgb(0xC4, 0x2B, 0x1C), Color32::WHITE),
            (_, true) => (pal.ink.gamma_multiply(0.08), pal.ink),
            _ => (Color32::TRANSPARENT, pal.ink),
        };
        let p = ui.painter();
        p.rect_filled(rect, 6.0, fill);
        let c = rect.center();
        let s = egui::Stroke::new(1.1, fg);
        match ctl {
            Ctl::Min => {
                p.hline(c.x - 5.0..=c.x + 5.0, c.y, s);
            }
            Ctl::Max => {
                if maximized {
                    p.rect_stroke(
                        Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(8.0, 8.0)),
                        1.0,
                        s,
                        egui::StrokeKind::Middle,
                    );
                    p.line_segment([c + vec2(-2.0, -4.5), c + vec2(4.5, -4.5)], s);
                    p.line_segment([c + vec2(4.5, -4.5), c + vec2(4.5, 2.0)], s);
                } else {
                    p.rect_stroke(Rect::from_center_size(c, vec2(9.0, 9.0)), 1.0, s, egui::StrokeKind::Middle);
                }
            }
            Ctl::Close => {
                p.line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], s);
                p.line_segment([c + vec2(4.5, -4.5), c + vec2(-4.5, 4.5)], s);
            }
        }
        if resp.clicked() {
            let cmd = match ctl {
                Ctl::Min => ViewportCommand::Minimized(true),
                Ctl::Max => ViewportCommand::Maximized(!maximized),
                Ctl::Close => ViewportCommand::Close,
            };
            ui.ctx().send_viewport_cmd(cmd);
        }
    }
}

/// Make `rect` behave like a title bar: drag to move, double-click to maximize.
/// Call *before* adding the widgets that sit on top of it.
pub fn drag_area(ui: &mut Ui, rect: Rect, id: egui::Id) {
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
}

/// Invisible resize handles along the window edges (frameless mode only).
pub fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let r = ctx.content_rect();
    let e = 5.0;
    let c = 12.0;
    let zones = [
        (
            Rect::from_min_max(r.left_top(), egui::pos2(r.left() + c, r.top() + c)),
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNorthWest,
        ),
        (
            Rect::from_min_max(egui::pos2(r.right() - c, r.top()), egui::pos2(r.right(), r.top() + c)),
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNorthEast,
        ),
        (
            Rect::from_min_max(egui::pos2(r.left(), r.bottom() - c), egui::pos2(r.left() + c, r.bottom())),
            ResizeDirection::SouthWest,
            CursorIcon::ResizeSouthWest,
        ),
        (
            Rect::from_min_max(egui::pos2(r.right() - c, r.bottom() - c), r.right_bottom()),
            ResizeDirection::SouthEast,
            CursorIcon::ResizeSouthEast,
        ),
        (
            Rect::from_min_max(r.left_top(), egui::pos2(r.left() + e, r.bottom())),
            ResizeDirection::West,
            CursorIcon::ResizeWest,
        ),
        (
            Rect::from_min_max(egui::pos2(r.right() - e, r.top()), r.right_bottom()),
            ResizeDirection::East,
            CursorIcon::ResizeEast,
        ),
        (
            Rect::from_min_max(r.left_top(), egui::pos2(r.right(), r.top() + e)),
            ResizeDirection::North,
            CursorIcon::ResizeNorth,
        ),
        (
            Rect::from_min_max(egui::pos2(r.left(), r.bottom() - e), r.right_bottom()),
            ResizeDirection::South,
            CursorIcon::ResizeSouth,
        ),
    ];
    egui::Area::new(egui::Id::unique("resize-edges"))
        .order(egui::Order::Debug)
        .fixed_pos(r.min)
        .interactable(true)
        .show(ctx, |ui| {
            for (i, (zone, dir, cursor)) in zones.into_iter().enumerate() {
                let resp = ui.interact(zone, egui::Id::unique(("resize", i)), Sense::drag());
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(cursor);
                }
                if resp.drag_started() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(dir));
                }
            }
        });
}
