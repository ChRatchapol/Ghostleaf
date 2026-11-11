//! Stealth reading: hide the moment the mouse leaves the window (optionally on
//! focus loss), reveal when it comes back; plus a global boss key.

use crate::style::Palette;
use egui::{Color32, Pos2, Rect};
use gl_core::{Stealth as StealthCfg, StealthStyle};
use std::sync::mpsc::Receiver;

#[cfg(not(target_os = "android"))]
type BossKey = (global_hotkey::GlobalHotKeyManager, global_hotkey::hotkey::HotKey);
#[cfg(target_os = "android")]
type BossKey = ();

pub struct Stealth {
    pub hidden: bool,
    pending_at: Option<f64>,
    by_boss: bool,
    minimized: bool,
    /// Ignore pointer-leave events until this time (e.g. right after a native file dialog).
    suppress_until: f64,
    hotkey: Option<BossKey>,
    /// Touch input seen (phones, tablets, touchscreens): fingers lifting off the
    /// screen also report "pointer gone", so leaving = leaving the app instead.
    touch: bool,
    hotkey_rx: Option<Receiver<()>>,
}

impl Stealth {
    pub fn new() -> Self {
        Self {
            hidden: false,
            pending_at: None,
            by_boss: false,
            minimized: false,
            suppress_until: 0.0,
            hotkey: None,
            touch: cfg!(target_os = "android"),
            hotkey_rx: None,
        }
    }

    pub fn suppress(&mut self, now: f64, secs: f64) {
        self.suppress_until = now + secs;
        self.pending_at = None;
    }

    /// No global hotkeys on Android.
    #[cfg(target_os = "android")]
    pub fn sync_boss_key(&mut self, _enabled: bool, _ctx: &egui::Context) {
        let _ = (&self.hotkey, &self.hotkey_rx);
    }

    /// Register / unregister the global boss key (Ctrl+Alt+H).
    #[cfg(not(target_os = "android"))]
    pub fn sync_boss_key(&mut self, enabled: bool, ctx: &egui::Context) {
        use global_hotkey::{
            GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
            hotkey::{Code, HotKey, Modifiers},
        };
        if enabled == self.hotkey.is_some() {
            return;
        }
        if !enabled {
            if let Some((m, hk)) = self.hotkey.take() {
                let _ = m.unregister(hk);
            }
            GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
            self.hotkey_rx = None;
            return;
        }
        let Ok(manager) = GlobalHotKeyManager::new() else { return };
        let hk = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyH);
        if manager.register(hk).is_err() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let id = hk.id();
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.id == id && e.state == HotKeyState::Pressed {
                let _ = tx.send(());
                ctx.request_repaint();
            }
        }));
        self.hotkey = Some((manager, hk));
        self.hotkey_rx = Some(rx);
    }

    fn hide(&mut self, ctx: &egui::Context, cfg: &StealthCfg) {
        if self.hidden {
            return;
        }
        self.hidden = true;
        self.pending_at = None;
        if cfg.style == StealthStyle::Minimize && !self.touch {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            self.minimized = true;
        }
    }

    fn reveal(&mut self, ctx: &egui::Context) {
        if !self.hidden {
            return;
        }
        self.hidden = false;
        self.by_boss = false;
        if self.minimized {
            self.minimized = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    /// Process this frame's input. Returns true if the UI should be covered.
    pub fn update(&mut self, ctx: &egui::Context, cfg: &StealthCfg) -> bool {
        let now = ctx.input(|i| i.time);
        // boss key works even with automatic hiding turned off
        let boss = self.hotkey_rx.as_ref().map(|rx| rx.try_iter().count() % 2 == 1).unwrap_or(false);
        if boss {
            if self.hidden {
                self.reveal(ctx);
            } else {
                self.hide(ctx, cfg);
                self.by_boss = true;
            }
        }
        if !cfg.enabled {
            if self.hidden && !self.by_boss {
                self.reveal(ctx);
            }
            return self.hidden;
        }
        let (gone, moved, clicked, focused, any_down, touched) = ctx.input(|i| {
            let mut gone = false;
            let mut moved = false;
            let mut clicked = false;
            let mut touched = false;
            for e in &i.events {
                match e {
                    egui::Event::PointerGone => gone = true,
                    egui::Event::PointerMoved(_) => moved = true,
                    egui::Event::PointerButton { pressed: true, .. } => clicked = true,
                    egui::Event::Touch {
                        phase: egui::TouchPhase::Start,
                        ..
                    } => touched = true,
                    _ => {}
                }
            }
            (gone, moved, clicked, i.viewport().focused, i.pointer.any_down(), touched)
        });
        self.touch |= touched;
        if self.touch {
            // Touch: hide when the app goes to the background (or loses focus);
            // when it comes back the page stays covered until you tap.
            if self.hidden {
                if !self.by_boss && (touched || clicked) && focused != Some(false) {
                    self.reveal(ctx);
                }
                return self.hidden;
            }
            if focused == Some(false) && now >= self.suppress_until {
                self.hide(ctx, cfg);
            }
            return self.hidden;
        }
        if self.hidden {
            if self.by_boss {
                return true;
            }
            let back = if cfg.style == StealthStyle::Minimize {
                focused == Some(true) && (moved || clicked)
            } else if cfg.reveal_on_click {
                clicked
            } else {
                moved || clicked
            };
            let focus_ok = !cfg.on_focus_loss || focused != Some(false);
            if back && focus_ok {
                self.reveal(ctx);
            }
            return self.hidden;
        }
        if now < self.suppress_until {
            return false;
        }
        if gone && !any_down {
            self.pending_at = Some(now + cfg.hide_delay_ms as f64 / 1000.0);
        }
        if moved && !gone {
            self.pending_at = None;
        }
        if cfg.on_focus_loss && focused == Some(false) {
            self.hide(ctx, cfg);
        }
        if let Some(at) = self.pending_at {
            if now >= at {
                self.hide(ctx, cfg);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.0)));
            }
        }
        self.hidden
    }
}

/// Paint the cover over the whole window.
pub fn paint_cover(ctx: &egui::Context, style: StealthStyle, pal: Palette, hint: &str) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::unique("stealth-cover")));
    match style {
        StealthStyle::Decoy => paint_decoy(&painter, rect),
        _ => {
            painter.rect_filled(rect, 0.0, pal.bg);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                hint,
                egui::FontId::proportional(12.0),
                pal.muted.gamma_multiply(0.35),
            );
        }
    }
}

/// A plain, neutral spreadsheet look-alike: grid, column letters, row numbers. No text content.
fn paint_decoy(painter: &egui::Painter, rect: Rect) {
    let bg = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    let head = Color32::from_rgb(0xF3, 0xF3, 0xF3);
    let grid = Color32::from_rgb(0xE1, 0xE1, 0xE1);
    let txt = Color32::from_rgb(0x66, 0x66, 0x66);
    painter.rect_filled(rect, 0.0, bg);
    let toolbar = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 34.0));
    painter.rect_filled(toolbar, 0.0, Color32::from_rgb(0xF8, 0xF8, 0xF8));
    painter.hline(toolbar.x_range(), toolbar.bottom(), egui::Stroke::new(1.0, grid));
    let (cw, rh, rw) = (96.0, 22.0, 42.0);
    let top = toolbar.bottom();
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), top + rh)),
        0.0,
        head,
    );
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.left() + rw, rect.bottom())),
        0.0,
        head,
    );
    let mut x = rect.left() + rw;
    let mut col = 0u8;
    while x < rect.right() {
        painter.vline(x, top..=rect.bottom(), egui::Stroke::new(1.0, grid));
        let letter = (b'A' + (col % 26)) as char;
        painter.text(
            Pos2::new(x + cw / 2.0, top + rh / 2.0),
            egui::Align2::CENTER_CENTER,
            letter,
            egui::FontId::proportional(11.0),
            txt,
        );
        x += cw;
        col += 1;
    }
    let mut y = top + rh;
    let mut row = 1;
    while y < rect.bottom() {
        painter.hline(rect.x_range(), y, egui::Stroke::new(1.0, grid));
        painter.text(
            Pos2::new(rect.left() + rw / 2.0, y + rh / 2.0),
            egui::Align2::CENTER_CENTER,
            row.to_string(),
            egui::FontId::proportional(11.0),
            txt,
        );
        y += rh;
        row += 1;
    }
}
