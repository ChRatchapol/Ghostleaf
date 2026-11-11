//! The reading view: page geometry, navigation, page turns (curl / slide /
//! none), texture management and the reader chrome.

use crate::curl_draw::{self, CurlPaint, SheetFrame};
use crate::i18n::Tr;
use crate::sound::{Sfx, Sound};
use crate::style::{self, Palette};
use crate::worker::{DocWorker, Req, Resp};
use egui::{Align2, Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use gl_core::{BookEntry, Bookmark, DisplayMode, Format, Location, Settings, Theme, TurnStyle};
use gl_curl::{Curl, Spring, V2, Velocity, should_complete};
use gl_formats::{LayoutInfo, LayoutRequest, RgbaImage};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const PAD: f32 = 28.0;
const TOP_BAR_H: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Blank,
    Page(usize),
}

pub struct LayoutState {
    pub id: u64,
    pub info: LayoutInfo,
    pub starts: Vec<Location>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    Fwd,
    Back,
}

struct Turn {
    dir: Dir,
    style: TurnStyle,
    /// The view whose outer page turns forward (for Back: the previous view).
    from_view: usize,
    to_view: usize,
    grab: V2,
    edge: bool,
    spring: Spring,
    dragging: bool,
    press_local: V2,
    press_p: V2,
    k: f32,
    vel: Velocity,
    auto: Option<(f64, f32)>,
    complete: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidePanel {
    Contents,
    Bookmarks,
}

pub enum Action {
    None,
    Close,
    OpenSettings,
}

pub struct Env<'a> {
    pub settings: &'a mut Settings,
    pub book: &'a mut BookEntry,
    pub sound: &'a mut Sound,
    pub t: &'static Tr,
    pub pal: Palette,
    pub settings_dirty: &'a mut bool,
    pub library_dirty: &'a mut bool,
    /// No OS title bar: the top bar doubles as one (drag + window controls).
    pub frameless: bool,
}

pub struct Reader {
    pub book_id: String,
    pub path: PathBuf,
    pub title: String,
    pub format: Option<Format>,
    pub reflow: bool,
    worker: DocWorker,
    pub layout: Option<LayoutState>,
    layout_seq: u64,
    sent_req: Option<LayoutRequest>,
    candidate: Option<(LayoutRequest, f64)>,
    view: usize,
    anchor: Location,
    anchor_applied: bool,
    tex: HashMap<usize, (u64, TextureHandle)>,
    /// The real paper colour of each rendered page (sampled from its corners).
    page_paper: HashMap<usize, Color32>,
    requested: HashSet<(usize, u64)>,
    theme_key: u64,
    turn: Option<Turn>,
    pub error: Option<String>,
    pub panel: Option<SidePanel>,
    wheel_acc: f32,
    pub cover: Option<RgbaImage>,
    chrome_until: f64,
    /// Top bar pinned open by a tap in the middle of the page (touch has no hover).
    chrome_pinned: bool,
    /// Phone-sized layout this frame.
    compact: bool,
    goto_text: String,
    font_list: Option<Vec<String>>,
    pending_action: Option<Action>,
}

fn slot_key(settings: &Settings) -> u64 {
    style::palette_key(&style::palette(settings)) ^ settings.pdf_dark_invert as u64
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Paper colour of a rendered page: the median of a few samples near its corners
/// (the margins), robust against a corner that happens to be printed on.
fn paper_of(img: &RgbaImage) -> Color32 {
    let (w, h) = (img.width.max(1), img.height.max(1));
    let mut s: Vec<[u8; 3]> = Vec::new();
    for (fx, fy) in [
        (0.02, 0.02),
        (0.98, 0.02),
        (0.02, 0.98),
        (0.98, 0.98),
        (0.5, 0.01),
        (0.5, 0.99),
        (0.01, 0.5),
        (0.99, 0.5),
    ] {
        let x = ((w as f32 * fx) as u32).min(w - 1);
        let y = ((h as f32 * fy) as u32).min(h - 1);
        let i = ((y * w + x) * 4) as usize;
        if let Some(p) = img.pixels.get(i..i + 3) {
            s.push([p[0], p[1], p[2]]);
        }
    }
    if s.is_empty() {
        return Color32::WHITE;
    }
    s.sort_by_key(|c| c[0] as u32 + c[1] as u32 + c[2] as u32);
    let m = s[s.len() / 2];
    Color32::from_rgb(m[0], m[1], m[2])
}

pub fn fit(aspect: f32, area: Rect) -> Rect {
    let (w, h) = if area.width() / area.height().max(1.0) > aspect {
        (area.height() * aspect, area.height())
    } else {
        (area.width(), area.width() / aspect.max(0.01))
    };
    Rect::from_center_size(area.center(), Vec2::new(w, h))
}

fn loc_num(l: Location) -> usize {
    match l {
        Location::Page(n) | Location::Text(n) | Location::Row(n) => n,
    }
}

impl Reader {
    pub fn open(book: &BookEntry, ctx: &egui::Context, settings: &Settings) -> Self {
        let worker = DocWorker::spawn(book.path.clone(), ctx.clone());
        if settings.save_covers && book.format == Format::Pdf {
            worker.send(Req::Cover { max_w: 240 });
        }
        let now = ctx.input(|i| i.time);
        Self {
            book_id: book.id.clone(),
            path: book.path.clone(),
            title: book.title.clone(),
            format: None,
            reflow: false,
            worker,
            layout: None,
            layout_seq: 0,
            sent_req: None,
            candidate: None,
            view: 0,
            anchor: book.location,
            anchor_applied: false,
            tex: HashMap::new(),
            page_paper: HashMap::new(),
            requested: HashSet::new(),
            theme_key: 0,
            turn: None,
            error: None,
            panel: None,
            wheel_acc: 0.0,
            cover: None,
            chrome_until: now + 2.5,
            chrome_pinned: false,
            compact: false,
            goto_text: String::new(),
            font_list: None,
            pending_action: None,
        }
    }

    // ---------- slots & views ----------

    fn page_count(&self) -> usize {
        self.layout.as_ref().map(|l| l.info.page_count).unwrap_or(0)
    }
    fn blank(book: &BookEntry, s: &Settings) -> bool {
        book.first_blank.unwrap_or(s.default_first_blank)
    }
    fn rtl(book: &BookEntry, s: &Settings) -> bool {
        book.rtl.unwrap_or(s.default_rtl)
    }
    fn n_slots(&self, blank: bool) -> usize {
        self.page_count() + blank as usize
    }
    fn slot(&self, s: usize, blank: bool) -> Option<Slot> {
        if s >= self.n_slots(blank) {
            return None;
        }
        Some(if blank {
            if s == 0 { Slot::Blank } else { Slot::Page(s - 1) }
        } else {
            Slot::Page(s)
        })
    }
    fn slot_of_page(page: usize, blank: bool) -> usize {
        page + blank as usize
    }
    fn normalize_view(&self, v: usize, spread: bool, blank: bool) -> usize {
        let n = self.n_slots(blank);
        let v = v.min(n.saturating_sub(1));
        if spread { v - v % 2 } else { v }
    }
    fn first_page_in_view(&self, view: usize, blank: bool) -> usize {
        match self.slot(view, blank) {
            Some(Slot::Page(p)) => p,
            _ => match self.slot(view + 1, blank) {
                Some(Slot::Page(p)) => p,
                _ => 0,
            },
        }
    }
    fn page_of(&self, loc: Location) -> usize {
        let Some(l) = &self.layout else { return 0 };
        if let Location::Page(p) = loc {
            return p.min(l.info.page_count.saturating_sub(1));
        }
        let x = loc_num(loc);
        let k = l.starts.partition_point(|s| loc_num(*s) <= x);
        if k == 0 {
            return 0;
        }
        let target = loc_num(l.starts[k - 1]);
        l.starts.iter().position(|s| loc_num(*s) == target).unwrap_or(k - 1)
    }

    // ---------- geometry ----------

    fn aspect_of(&self, slot: Option<Slot>, fallback: f32) -> f32 {
        match (slot, &self.layout) {
            (Some(Slot::Page(p)), Some(l)) if !l.info.aspects.is_empty() => *l.info.aspects.get(p).unwrap_or(&fallback),
            _ => fallback,
        }
    }

    /// Page rectangles for a view: (first-in-reading-order, second). In single mode only the first.
    fn view_rects(&self, area: Rect, spread: bool, rtl: bool, view: usize, blank: bool) -> (Rect, Option<Rect>) {
        // pages sit below the (auto-hiding) top bar, so their top corners can always be grabbed
        let pad = if self.compact { 8.0 } else { PAD };
        let inner = Rect::from_min_max(
            Pos2::new(area.left() + pad, area.top() + TOP_BAR_H + 6.0),
            Pos2::new(area.right() - pad, area.bottom() - pad * 0.6),
        );
        let base = if self.reflow {
            let per_w = if spread { inner.width() / 2.0 } else { inner.width() };
            (per_w.min(inner.height() * 0.74)) / inner.height().max(1.0)
        } else {
            let a0 = self.aspect_of(self.slot(view, blank), 0.0);
            let a1 = self.aspect_of(self.slot(view + 1, blank), 0.0);
            let fallback = self.aspect_of(Some(Slot::Page(0)), 0.707);
            if a0 > 0.0 {
                a0
            } else if a1 > 0.0 && spread {
                a1
            } else {
                fallback
            }
        };
        if !spread {
            return (fit(base, inner), None);
        }
        let r = fit(base * 2.0, inner);
        let left = Rect::from_min_max(r.min, Pos2::new(r.center().x, r.max.y));
        let right = Rect::from_min_max(Pos2::new(r.center().x, r.min.y), r.max);
        if rtl { (right, Some(left)) } else { (left, Some(right)) }
    }

    fn is_spread(settings: &Settings, area: Rect) -> bool {
        match settings.display {
            DisplayMode::Single => false,
            DisplayMode::Spread => true,
            DisplayMode::Auto => area.width() / area.height().max(1.0) > 1.25,
        }
    }

    // ---------- worker I/O ----------

    fn poll(&mut self, ctx: &egui::Context, env: &mut Env) {
        while let Ok(r) = self.worker.rx.try_recv() {
            match r {
                Resp::Opened { title, format, reflow } => {
                    if env.book.title.is_empty() || env.book.title == gl_formats::title_from_path(&self.path) {
                        env.book.title = title.clone();
                        *env.library_dirty = true;
                    }
                    self.title = title;
                    self.format = Some(format);
                    self.reflow = reflow;
                }
                Resp::Layout { id, info, starts } => {
                    if id != self.layout_seq {
                        continue;
                    }
                    self.layout = Some(LayoutState { id, info, starts });
                    self.requested.clear();
                    if self.reflow {
                        self.tex.clear();
                    }
                    self.anchor_applied = false;
                }
                Resp::Page { page, key, image } => {
                    let img = egui::ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &image.pixels);
                    let h = ctx.load_texture(format!("page-{page}"), img, TextureOptions::LINEAR);
                    self.page_paper.insert(page, paper_of(&image));
                    self.tex.insert(page, (key, h));
                    self.requested.remove(&(page, key));
                }
                Resp::Cover(img) => self.cover = Some(img),
                Resp::Error(e) => self.error = Some(e),
            }
        }
    }

    fn request_layout(&mut self, req: LayoutRequest) {
        self.layout_seq += 1;
        self.sent_req = Some(req.clone());
        self.worker.send(Req::Layout { id: self.layout_seq, req });
    }

    /// Desired texture key and pixel size for a page drawn in `rect`.
    fn tex_spec(&self, rect: Rect, ppp: f32) -> (u64, u32, u32) {
        if self.reflow {
            let r = self.sent_req.as_ref().unwrap();
            let lid = self.layout.as_ref().map(|l| l.id).unwrap_or(0);
            (lid << 8 ^ self.theme_key, r.page_w, r.page_h)
        } else {
            let q = |v: f32| (((v * ppp) / 64.0).ceil() as u32 * 64).max(64);
            let (w, h) = (q(rect.width()), q(rect.height()));
            ((w as u64) << 40 ^ (h as u64) << 16 ^ self.theme_key, w, h)
        }
    }

    fn ensure_page(&mut self, page: usize, rect: Rect, ppp: f32, settings: &Settings) {
        let Some(lay) = &self.layout else { return };
        if page >= lay.info.page_count {
            return;
        }
        let (key, w, h) = self.tex_spec(rect, ppp);
        if self.tex.get(&page).map(|t| t.0) == Some(key) || self.requested.contains(&(page, key)) {
            return;
        }
        self.requested.insert((page, key));
        let theme = style::render_theme(settings);
        self.worker.send(Req::Render {
            layout_id: lay.id,
            page,
            w,
            h,
            theme,
            key,
        });
    }

    // ---------- navigation ----------

    fn step(spread: bool) -> usize {
        if spread { 2 } else { 1 }
    }

    fn next_view(&self, view: usize, spread: bool, blank: bool) -> Option<usize> {
        let v = view + Self::step(spread);
        (v < self.n_slots(blank)).then_some(v)
    }
    fn prev_view(&self, view: usize, spread: bool) -> Option<usize> {
        view.checked_sub(Self::step(spread))
    }

    fn set_view(&mut self, v: usize, env: &mut Env, blank: bool) {
        self.view = v;
        let p = self.first_page_in_view(v, blank);
        if let Some(l) = &self.layout
            && let Some(loc) = l.starts.get(p)
        {
            self.anchor = *loc;
            env.book.location = *loc;
            env.book.progress = if l.info.page_count > 1 {
                p as f32 / (l.info.page_count - 1) as f32
            } else {
                1.0
            };
            *env.library_dirty = true;
        }
    }

    fn start_auto(&mut self, dir: Dir, spread: bool, blank: bool, style: TurnStyle, now: f64, frame_w: f32, frame_h: f32, env: &mut Env) {
        if self.turn.is_some() {
            return;
        }
        let (from, to) = match dir {
            Dir::Fwd => match self.next_view(self.view, spread, blank) {
                Some(n) => (self.view, n),
                None => return,
            },
            Dir::Back => match self.prev_view(self.view, spread) {
                Some(p) => (p, self.view),
                None => return,
            },
        };
        if style == TurnStyle::None {
            let v = if dir == Dir::Fwd { to } else { from };
            self.set_view(v, env, blank);
            return;
        }
        if env.settings.sound {
            env.sound.play(Sfx::Flip, env.settings.volume);
        }
        let grab = V2::new(frame_w, frame_h);
        let dur = if style == TurnStyle::Curl { 0.62 } else { 0.32 };
        self.turn = Some(Turn {
            dir,
            style,
            from_view: from,
            to_view: to,
            grab,
            edge: false,
            spring: Spring::new(grab, 15.0),
            dragging: false,
            press_local: V2::default(),
            press_p: V2::default(),
            k: 1.0,
            vel: Velocity::default(),
            auto: Some((now, dur)),
            complete: Some(true),
        });
    }

    /// P (finger) position along the scripted auto-turn path, in the from-view's sheet frame.
    fn auto_p(dir: Dir, t: f32, w: f32, h: f32) -> V2 {
        let e = ease_in_out(t);
        let e = if dir == Dir::Fwd { e } else { 1.0 - e };
        V2::new(w - 2.0 * w * e, h - (std::f32::consts::PI * e).sin() * h * 0.16)
    }

    // ---------- main UI ----------

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env) -> Action {
        let ctx = ui.ctx().clone();
        self.poll(&ctx, env);
        let now = ctx.input(|i| i.time);
        let ppp = ctx.pixels_per_point();
        let theme_key = slot_key(env.settings);
        if theme_key != self.theme_key {
            self.theme_key = theme_key;
            self.requested.clear();
        }
        let mut action = Action::None;
        self.compact = style::compact(&ctx);
        // Android back button (and Escape for panels): close what's open, else go back to the library.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)) {
            if self.panel.is_some() {
                self.panel = None;
            } else if self.chrome_pinned {
                self.chrome_pinned = false;
            } else {
                return Action::Close;
            }
        }
        if self.panel.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.panel = None;
        }

        // side panel
        if let Some(panel) = self.panel {
            egui::Panel::right("reader-side")
                .resizable(false)
                .exact_size(280.0_f32.min(ctx.content_rect().width() * 0.86))
                .frame(egui::Frame::new().fill(env.pal.panel).inner_margin(egui::Margin::same(16)))
                .show(ui, |ui| self.side_panel(ui, panel, env));
        }

        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0.0, env.pal.bg);

        if let Some(err) = &self.error {
            ui.painter().text(
                area.center(),
                Align2::CENTER_CENTER,
                err,
                egui::FontId::proportional(15.0),
                env.pal.muted,
            );
            if self.top_bar(ui, area, env, true) {
                action = Action::Close;
            }
            return action;
        }

        let spread = Self::is_spread(env.settings, area);
        let rtl = Self::rtl(env.book, env.settings);
        let blank = Self::blank(env.book, env.settings);

        // ---- layout requests ----
        if self.format.is_some() {
            let (r0, _) = self.view_rects(area, spread, rtl, self.view, blank);
            let req = LayoutRequest {
                page_w: if self.reflow { (r0.width() * ppp).round() as u32 } else { 1 },
                page_h: if self.reflow { (r0.height() * ppp).round() as u32 } else { 1 },
                scale: ppp,
                style: env.settings.text.clone(),
            };
            if self.sent_req.is_none() {
                self.request_layout(req);
            } else if self.reflow && self.sent_req.as_ref() != Some(&req) {
                // debounce resizes / slider drags
                let stable = match &self.candidate {
                    Some((c, since)) if *c == req => now - since > 0.18,
                    _ => {
                        self.candidate = Some((req.clone(), now));
                        false
                    }
                };
                if stable && self.turn.is_none() {
                    self.candidate = None;
                    self.request_layout(req);
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(60));
                }
            }
        }

        let Some(lay_count) = self.layout.as_ref().map(|l| l.info.page_count) else {
            ui.painter().text(
                area.center(),
                Align2::CENTER_CENTER,
                env.t.loading,
                egui::FontId::proportional(15.0),
                env.pal.muted,
            );
            if self.top_bar(ui, area, env, true) {
                action = Action::Close;
            }
            return action;
        };
        if !self.anchor_applied {
            self.anchor_applied = true;
            let p = self.page_of(self.anchor);
            self.view = self.normalize_view(Self::slot_of_page(p, blank), spread, blank);
        }
        let normalized = self.normalize_view(self.view, spread, blank);
        if normalized != self.view && self.turn.is_none() {
            self.view = normalized;
        }

        // sheet frame for the current geometry
        let (first_r, second_r) = self.view_rects(area, spread, rtl, self.view, blank);
        let outer = second_r.unwrap_or(first_r);
        let frame = SheetFrame { rect: outer, rtl };
        let (fw, fh) = (frame.w(), frame.h());
        let k = if spread { 1.0 } else { 2.0 };

        // ---- input ----
        // The top strip belongs to the bar (or its handle): never starts a page turn.
        // While a menu is open, a tap outside only closes it.
        let strip = Rect::from_min_size(area.min, Vec2::new(area.width(), TOP_BAR_H));
        let pages_rect = Rect::from_min_max(Pos2::new(area.left(), strip.bottom()), area.max);
        let menu_open = egui::Popup::is_any_open(&ctx);
        let sense = if menu_open { Sense::hover() } else { Sense::click_and_drag() };
        let resp = ui.interact(pages_rect, ui.scope_id().with("pages"), sense);
        let strip_resp = ui.interact(strip, ui.scope_id().with("strip"), Sense::click());
        if strip_resp.clicked() {
            self.chrome_pinned = true;
        }
        let style = env.settings.turn_style;
        if resp.drag_started()
            && self.turn.is_none()
            && style != TurnStyle::None
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let local = frame.to_local(pos);
            let fwd = if spread { local.x > 0.0 } else { local.x > fw * 0.5 };
            let target = if fwd {
                self.next_view(self.view, spread, blank).map(|n| (self.view, n))
            } else {
                self.prev_view(self.view, spread).map(|p| (p, self.view))
            };
            if let Some((from, to)) = target {
                let y = local.y.clamp(0.0, fh);
                let (grab, edge) = if y < fh * 0.2 {
                    (V2::new(fw, 0.0), false)
                } else if y > fh * 0.8 {
                    (V2::new(fw, fh), false)
                } else {
                    (V2::new(fw, y), true)
                };
                let p0 = if fwd { grab } else { V2::new(-fw, grab.y) };
                let mut vel = Velocity::default();
                vel.push(p0, now);
                if env.settings.sound {
                    env.sound.play(Sfx::Peel, env.settings.volume);
                }
                self.turn = Some(Turn {
                    dir: if fwd { Dir::Fwd } else { Dir::Back },
                    style,
                    from_view: from,
                    to_view: to,
                    grab,
                    edge,
                    spring: Spring::new(p0, 16.0),
                    dragging: true,
                    press_local: local,
                    press_p: p0,
                    k,
                    vel,
                    auto: None,
                    complete: None,
                });
            }
        }
        if let Some(t) = &mut self.turn
            && t.dragging
        {
            if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                let local = frame.to_local(pos);
                let p = t.press_p + (local - t.press_local) * t.k;
                t.spring.pos = p;
                t.spring.target = p;
                t.spring.vel = V2::default();
                t.vel.push(p, now);
            }
            if !ctx.input(|i| i.pointer.primary_down()) {
                t.dragging = false;
                let c = Curl {
                    w: fw,
                    h: fh,
                    grab: t.grab,
                    drag: t.spring.pos,
                    max_radius: fw * 0.09,
                    edge: t.edge,
                };
                let prog = c.progress();
                let done = match t.dir {
                    Dir::Fwd => should_complete(prog, t.vel.v.x, fw),
                    Dir::Back => should_complete(1.0 - prog, -t.vel.v.x, fw),
                };
                t.complete = Some(done);
                let turned = V2::new(-fw, t.grab.y);
                t.spring.target = match (t.dir, done) {
                    (Dir::Fwd, true) | (Dir::Back, false) => turned,
                    _ => t.grab,
                };
                t.spring.vel = t.vel.v * 0.6;
                if done && env.settings.sound {
                    env.sound.play(Sfx::Land, env.settings.volume);
                }
            }
        }
        if resp.clicked()
            && self.turn.is_none()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let rel = (pos.x - area.left()) / area.width();
            let (left_dir, right_dir) = if rtl { (Dir::Fwd, Dir::Back) } else { (Dir::Back, Dir::Fwd) };
            if rel > 0.62 {
                self.start_auto(right_dir, spread, blank, style, now, fw, fh, env);
            } else if rel < 0.38 {
                self.start_auto(left_dir, spread, blank, style, now, fw, fh, env);
            } else {
                self.chrome_pinned = !self.chrome_pinned;
                self.chrome_until = 0.0;
            }
        }
        // keyboard & wheel
        if !ctx.egui_wants_keyboard_input() {
            let (next_key, prev_key) = if rtl {
                (egui::Key::ArrowLeft, egui::Key::ArrowRight)
            } else {
                (egui::Key::ArrowRight, egui::Key::ArrowLeft)
            };
            let (n, p, home, end, bm, mute) = ctx.input(|i| {
                (
                    i.key_pressed(next_key) || i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::Space),
                    i.key_pressed(prev_key) || i.key_pressed(egui::Key::PageUp),
                    i.key_pressed(egui::Key::Home),
                    i.key_pressed(egui::Key::End),
                    i.key_pressed(egui::Key::B),
                    i.key_pressed(egui::Key::M),
                )
            });
            if n {
                self.start_auto(Dir::Fwd, spread, blank, style, now, fw, fh, env);
            }
            if p {
                self.start_auto(Dir::Back, spread, blank, style, now, fw, fh, env);
            }
            if home && self.turn.is_none() {
                self.set_view(0, env, blank);
            }
            if end && self.turn.is_none() {
                let v = self.normalize_view(self.n_slots(blank).saturating_sub(1), spread, blank);
                self.set_view(v, env, blank);
            }
            if bm {
                self.toggle_bookmark(env, blank);
            }
            if mute {
                env.settings.sound = !env.settings.sound;
                *env.settings_dirty = true;
            }
        }
        if resp.hovered() {
            let wheel: f32 = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, unit, .. } => Some(match unit {
                            egui::MouseWheelUnit::Point => delta.y,
                            egui::MouseWheelUnit::Line => delta.y * 40.0,
                            egui::MouseWheelUnit::Page => delta.y * 400.0,
                        }),
                        _ => None,
                    })
                    .sum()
            });
            self.wheel_acc += wheel;
            if self.wheel_acc.abs() > 80.0 && self.turn.is_none() {
                let d = if self.wheel_acc < 0.0 { Dir::Fwd } else { Dir::Back };
                self.wheel_acc = 0.0;
                self.start_auto(d, spread, blank, style, now, fw, fh, env);
            }
        }

        // ---- animate ----
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        let mut finished: Option<usize> = None;
        if let Some(t) = &mut self.turn {
            if let Some((t0, dur)) = t.auto {
                let x = ((now - t0) as f32 / dur).min(1.0);
                t.spring.pos = Self::auto_p(t.dir, x, fw, fh);
                if x >= 1.0 {
                    finished = Some(if t.dir == Dir::Fwd { t.to_view } else { t.from_view });
                }
            } else if !t.dragging {
                t.spring.step(dt);
                if t.spring.settled(0.8) {
                    finished = Some(match (t.dir, t.complete) {
                        (Dir::Fwd, Some(true)) => t.to_view,
                        (Dir::Back, Some(true)) => t.from_view,
                        (Dir::Fwd, _) => t.from_view,
                        (Dir::Back, _) => t.to_view,
                    });
                }
            }
            ctx.request_repaint();
        }
        if let Some(v) = finished {
            self.turn = None;
            self.set_view(v, env, blank);
        }

        // ---- textures ----
        let mut needed: Vec<(usize, Rect)> = Vec::new();
        let want_view = |v: usize, me: &Self, needed: &mut Vec<(usize, Rect)>| {
            let (a, b) = me.view_rects(area, spread, rtl, v, blank);
            for (s, r) in [(me.slot(v, blank), Some(a)), (if spread { me.slot(v + 1, blank) } else { None }, b)] {
                if let (Some(Slot::Page(p)), Some(r)) = (s, r) {
                    needed.push((p, r));
                }
            }
        };
        // prefetch first (lowest priority), visible last
        if let Some(n) = self.next_view(self.view, spread, blank) {
            want_view(n, self, &mut needed);
        }
        if let Some(p) = self.prev_view(self.view, spread) {
            want_view(p, self, &mut needed);
        }
        if let Some(t) = &self.turn {
            want_view(t.to_view, self, &mut needed);
            want_view(t.from_view, self, &mut needed);
        }
        want_view(self.view, self, &mut needed);
        for (p, r) in needed {
            self.ensure_page(p, r, ppp, env.settings);
        }
        // evict far pages
        let center = self.first_page_in_view(self.view, blank);
        self.tex.retain(|p, _| p.abs_diff(center) <= 8);

        // ---- draw ----
        let painter = ui.painter_at(area);
        let dark = env.pal.dark;
        match &self.turn {
            Some(t) if t.style == TurnStyle::Curl => {
                let c = Curl {
                    w: fw,
                    h: fh,
                    grab: t.grab,
                    drag: t.spring.pos,
                    max_radius: fw * 0.09,
                    edge: t.edge,
                };
                let cm = c.mesh(56, 48);
                let (fv, tv) = (t.from_view, t.to_view);
                let (f_first, f_second) = self.view_rects(area, spread, rtl, fv, blank);
                if spread {
                    let spread_rect = f_first.union(f_second.unwrap());
                    curl_draw::page_shadow(&painter, spread_rect, dark);
                    self.draw_slot(&painter, self.slot(fv, blank), f_first, env.pal);
                    self.draw_slot(&painter, self.slot(tv + 1, blank), f_second.unwrap(), env.pal);
                    curl_draw::gutter(&painter, spread_rect, dark);
                    curl_draw::draw_under_shadow(&painter.with_clip_rect(f_second.unwrap()), &frame, &cm, dark);
                    let paint = CurlPaint {
                        front: self.slot_tex(self.slot(fv + 1, blank)),
                        back: self.slot_tex(self.slot(tv, blank)),
                        paper: self.slot_paper(self.slot(fv + 1, blank), env.pal),
                        dark,
                        gutter: Some(curl_draw::gutter_params(spread_rect, dark)),
                    };
                    curl_draw::draw_sheet(&painter.with_clip_rect(spread_rect.expand(2.0)), &frame, &cm, &paint);
                } else {
                    curl_draw::page_shadow(&painter, f_first, dark);
                    self.draw_slot(&painter, self.slot(tv, blank), f_first, env.pal);
                    let clip = painter.with_clip_rect(f_first);
                    curl_draw::draw_under_shadow(&clip, &frame, &cm, dark);
                    let paint = CurlPaint {
                        front: self.slot_tex(self.slot(fv, blank)),
                        back: None,
                        paper: self.slot_paper(self.slot(fv, blank), env.pal),
                        dark,
                        gutter: None,
                    };
                    curl_draw::draw_sheet(&clip, &frame, &cm, &paint);
                }
            }
            Some(t) => {
                // slide
                let c = Curl {
                    w: fw,
                    h: fh,
                    grab: t.grab,
                    drag: t.spring.pos,
                    max_radius: 0.0,
                    edge: true,
                };
                let p = c.progress();
                let width = area.width();
                let sign = if rtl { 1.0 } else { -1.0 };
                let (fv, tv) = (t.from_view, t.to_view);
                self.draw_view(&painter, area, spread, rtl, fv, blank, Vec2::new(sign * p * width, 0.0), env.pal);
                self.draw_view(
                    &painter,
                    area,
                    spread,
                    rtl,
                    tv,
                    blank,
                    Vec2::new(-sign * (1.0 - p) * width, 0.0),
                    env.pal,
                );
            }
            None => self.draw_view(&painter, area, spread, rtl, self.view, blank, Vec2::ZERO, env.pal),
        }

        // ---- chrome ----
        let hover_top = ctx
            .input(|i| i.pointer.hover_pos())
            .map(|p| p.y < area.top() + TOP_BAR_H)
            .unwrap_or(false);
        let show = hover_top || self.chrome_pinned || now < self.chrome_until || self.panel.is_some() || egui::Popup::is_any_open(&ctx);
        if show && self.top_bar(ui, area, env, false) {
            action = Action::Close;
        }
        if !show {
            // a small tab that brings the bar back (tapping anywhere in the strip works too)
            let c = env.pal.muted;
            let tab = Rect::from_center_size(Pos2::new(area.center().x, area.top() + 14.0), Vec2::new(44.0, 20.0));
            ui.painter().rect_filled(tab, 10.0, env.pal.panel.gamma_multiply(0.85));
            crate::icons::draw(
                ui.painter(),
                Rect::from_center_size(tab.center(), Vec2::splat(18.0)),
                crate::icons::Icon::ChevronDown,
                c,
            );
            strip_resp.on_hover_text(env.t.show_controls);
        }
        if show {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            if let Some(a) = self.pending_action.take() {
                action = a;
            }
        }
        let _ = lay_count;
        action
    }

    /// Paper colour for a slot: the page's own (PDFs are usually white whatever the theme),
    /// the first page's for the blank flyleaf, else the theme's.
    fn slot_paper(&self, slot: Option<Slot>, pal: Palette) -> Color32 {
        let p = match slot {
            Some(Slot::Page(p)) => p,
            _ => 0,
        };
        self.page_paper
            .get(&p)
            .or_else(|| self.page_paper.values().next())
            .copied()
            .unwrap_or(pal.paper)
    }

    fn slot_tex(&self, slot: Option<Slot>) -> Option<egui::TextureId> {
        match slot {
            Some(Slot::Page(p)) => self.tex.get(&p).map(|t| t.1.id()),
            _ => None,
        }
    }

    fn draw_slot(&self, painter: &egui::Painter, slot: Option<Slot>, rect: Rect, pal: Palette) {
        match slot {
            None => {}
            Some(Slot::Blank) => {
                painter.rect_filled(rect, 0.0, self.slot_paper(None, pal));
            }
            Some(Slot::Page(p)) => {
                painter.rect_filled(rect, 0.0, self.slot_paper(Some(Slot::Page(p)), pal));
                if let Some((_, h)) = self.tex.get(&p) {
                    let r = if self.reflow {
                        rect
                    } else {
                        let a = h.size()[0] as f32 / h.size()[1].max(1) as f32;
                        fit(a, rect)
                    };
                    painter.image(h.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_view(
        &self,
        painter: &egui::Painter,
        area: Rect,
        spread: bool,
        rtl: bool,
        view: usize,
        blank: bool,
        offset: Vec2,
        pal: Palette,
    ) {
        let (a, b) = self.view_rects(area, spread, rtl, view, blank);
        let (a, b) = (a.translate(offset), b.map(|r| r.translate(offset)));
        let s0 = self.slot(view, blank);
        let s1 = if spread { self.slot(view + 1, blank) } else { None };
        let mut bounds: Option<Rect> = None;
        for (s, r) in [(s0, Some(a)), (s1, b)] {
            if let (Some(_), Some(r)) = (s, r) {
                bounds = Some(bounds.map_or(r, |x| x.union(r)));
            }
        }
        if let Some(bd) = bounds {
            curl_draw::page_shadow(painter, bd, pal.dark);
        }
        self.draw_slot(painter, s0, a, pal);
        if let Some(b) = b {
            self.draw_slot(painter, s1, b, pal);
            if s1.is_some() {
                curl_draw::gutter(painter, a.union(b), pal.dark);
            }
        }
    }

    fn toggle_bookmark(&mut self, env: &mut Env, blank: bool) {
        let p = self.first_page_in_view(self.view, blank);
        let Some(loc) = self.layout.as_ref().and_then(|l| l.starts.get(p).copied()) else {
            return;
        };
        if let Some(i) = env.book.bookmarks.iter().position(|b| self.page_of(b.location) == p) {
            env.book.bookmarks.remove(i);
        } else {
            env.book.bookmarks.push(Bookmark {
                location: loc,
                label: format!("{} {}", env.t.page, p + 1),
                created: gl_core::now_secs(),
            });
            env.book.bookmarks.sort_by_key(|b| loc_num(b.location));
        }
        *env.library_dirty = true;
    }

    fn current_page(&self, blank: bool) -> usize {
        self.first_page_in_view(self.view, blank)
    }

    fn go_to_page(&mut self, page: usize, env: &mut Env) {
        let blank = Self::blank(env.book, env.settings);
        self.turn = None;
        let v = Self::slot_of_page(page, blank);
        self.view = v; // normalized next frame
        self.set_view(v, env, blank);
        self.anchor_applied = true;
    }
}

// ---------- chrome ----------

impl Reader {
    /// Returns true when the user asked to go back to the library.
    fn top_bar(&mut self, ui: &mut egui::Ui, area: Rect, env: &mut Env, minimal: bool) -> bool {
        let mut back = false;
        let bar = Rect::from_min_size(area.min, Vec2::new(area.width(), TOP_BAR_H));
        let pal = env.pal;
        egui::Area::new(ui.scope_id().with("topbar"))
            .fixed_pos(bar.min)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(bar.size());
                ui.set_max_size(bar.size());
                if env.frameless {
                    crate::titlebar::drag_area(ui, bar, ui.scope_id().with("drag"));
                }
                ui.painter().rect_filled(bar, 0.0, pal.panel.gamma_multiply(0.94));
                ui.painter().hline(bar.x_range(), bar.bottom(), egui::Stroke::new(1.0, pal.line));
                let inner = bar.shrink2(Vec2::new(12.0, 8.0));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        if crate::icons::button(ui, crate::icons::Icon::Back, false, env.t.library).clicked() {
                            back = true;
                        }
                        if self.compact && !minimal {
                            // phone: back · title · page · ⋯ (everything else lives in the menu)
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if crate::icons::button(ui, crate::icons::Icon::ChevronUp, false, env.t.hide_controls).clicked() {
                                    self.chrome_pinned = false;
                                    self.chrome_until = 0.0;
                                }
                                let r = crate::icons::button(ui, crate::icons::Icon::More, false, env.t.more);
                                egui::Popup::menu(&r)
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                    .show(|ui| self.compact_menu(ui, env));
                                let blank = Self::blank(env.book, env.settings);
                                let total = self.page_count();
                                let page = self.current_page(blank);
                                ui.label(
                                    egui::RichText::new(format!("{} / {}", (page + 1).min(total.max(1)), total))
                                        .color(env.pal.muted)
                                        .small(),
                                );
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(&self.title).font(style::semibold(14.0))).truncate());
                                });
                            });
                            return;
                        }
                        ui.add(egui::Label::new(egui::RichText::new(&self.title).font(style::semibold(14.0))).truncate());
                        if minimal {
                            return;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            self.bar_controls(ui, env);
                            if self.chrome_pinned
                                && crate::icons::button(ui, crate::icons::Icon::ChevronUp, false, env.t.hide_controls).clicked()
                            {
                                self.chrome_pinned = false;
                                self.chrome_until = 0.0;
                            }
                        });
                    },
                );
            });
        back
    }

    /// Phone menu: every reading control, with labels, in one popup.
    fn compact_menu(&mut self, ui: &mut egui::Ui, env: &mut Env) {
        use crate::icons::{Icon, button as icon};
        let t = env.t;
        let blank = Self::blank(env.book, env.settings);
        ui.set_min_width(250.0);
        ui.horizontal(|ui| {
            if icon(ui, Icon::Contents, false, t.contents).clicked() {
                self.panel = Some(SidePanel::Contents);
                ui.close();
            }
            let page = self.current_page(blank);
            let marked = env.book.bookmarks.iter().any(|b| self.page_of(b.location) == page);
            if icon(
                ui,
                if marked { Icon::BookmarkFilled } else { Icon::Bookmark },
                false,
                t.add_bookmark,
            )
            .clicked()
            {
                self.toggle_bookmark(env, blank);
            }
            if icon(ui, if env.settings.sound { Icon::SoundOn } else { Icon::SoundOff }, false, t.sound).clicked() {
                env.settings.sound = !env.settings.sound;
                *env.settings_dirty = true;
            }
            if icon(ui, Icon::Theme, false, t.theme).clicked() {
                let all = Theme::ALL;
                let i = all.iter().position(|x| *x == env.settings.theme).unwrap_or(0);
                env.settings.theme = all[(i + 1) % all.len()];
                *env.settings_dirty = true;
            }
            if icon(ui, Icon::Settings, false, t.settings).clicked() {
                self.pending_action = Some(Action::OpenSettings);
                ui.close();
            }
        });
        ui.separator();
        ui.label(egui::RichText::new(t.page_turn).small());
        ui.horizontal_wrapped(|ui| {
            for (v, l) in [(TurnStyle::Curl, t.curl), (TurnStyle::Slide, t.slide), (TurnStyle::None, t.none)] {
                if ui.selectable_value(&mut env.settings.turn_style, v, l).changed() {
                    *env.settings_dirty = true;
                }
            }
        });
        ui.label(egui::RichText::new(t.display).small());
        ui.horizontal_wrapped(|ui| {
            for (v, l) in [
                (DisplayMode::Single, t.single),
                (DisplayMode::Spread, t.spread),
                (DisplayMode::Auto, t.auto),
            ] {
                if ui.selectable_value(&mut env.settings.display, v, l).changed() {
                    *env.settings_dirty = true;
                }
            }
        });
        let rtl = Self::rtl(env.book, env.settings);
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(!rtl, t.ltr).clicked() && rtl {
                env.book.rtl = Some(false);
                *env.library_dirty = true;
            }
            if ui.selectable_label(rtl, t.rtl).clicked() && !rtl {
                env.book.rtl = Some(true);
                *env.library_dirty = true;
            }
        });
        let mut b = blank;
        if ui.checkbox(&mut b, t.first_blank).changed() {
            env.book.first_blank = Some(b);
            *env.library_dirty = true;
        }
        if self.reflow {
            ui.separator();
            ui.collapsing(t.typography, |ui| {
                let fonts = self.font_list.get_or_insert_with(gl_formats::engine::font_families).clone();
                if crate::panels::typography(ui, env.settings, t, &fonts) {
                    *env.settings_dirty = true;
                }
            });
        }
    }

    fn bar_controls(&mut self, ui: &mut egui::Ui, env: &mut Env) {
        let t = env.t;
        let blank = Self::blank(env.book, env.settings);
        use crate::icons::{Icon, button as icon};
        if env.frameless {
            crate::titlebar::window_controls(ui, env.pal);
            ui.add_space(6.0);
        }
        if icon(ui, Icon::Settings, false, t.settings).clicked() {
            self.pending_action = Some(Action::OpenSettings);
        }
        if icon(ui, Icon::Contents, self.panel.is_some(), t.contents).clicked() {
            self.panel = if self.panel == Some(SidePanel::Contents) {
                None
            } else {
                Some(SidePanel::Contents)
            };
        }
        let page = self.current_page(blank);
        let marked = env.book.bookmarks.iter().any(|b| self.page_of(b.location) == page);
        if icon(
            ui,
            if marked { Icon::BookmarkFilled } else { Icon::Bookmark },
            false,
            t.add_bookmark,
        )
        .clicked()
        {
            self.toggle_bookmark(env, blank);
        }
        if self.reflow {
            let r = icon(ui, Icon::Text, false, t.typography);
            egui::Popup::from_toggle_button_response(&r)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(260.0);
                    let fonts = self.font_list.get_or_insert_with(gl_formats::engine::font_families).clone();
                    if crate::panels::typography(ui, env.settings, t, &fonts) {
                        *env.settings_dirty = true;
                    }
                });
        }
        // theme cycle
        if icon(ui, Icon::Theme, false, t.theme).clicked() {
            let all = Theme::ALL;
            let i = all.iter().position(|x| *x == env.settings.theme).unwrap_or(0);
            env.settings.theme = all[(i + 1) % all.len()];
            *env.settings_dirty = true;
        }
        if icon(ui, if env.settings.sound { Icon::SoundOn } else { Icon::SoundOff }, false, t.sound).clicked() {
            env.settings.sound = !env.settings.sound;
            *env.settings_dirty = true;
        }
        let turn_label = match env.settings.turn_style {
            TurnStyle::Curl => Icon::TurnCurl,
            TurnStyle::Slide => Icon::TurnSlide,
            TurnStyle::None => Icon::TurnNone,
        };
        let r = icon(ui, turn_label, false, t.page_turn);
        egui::Popup::menu(&r).show(|ui| {
            for (v, l) in [(TurnStyle::Curl, t.curl), (TurnStyle::Slide, t.slide), (TurnStyle::None, t.none)] {
                if ui.selectable_value(&mut env.settings.turn_style, v, l).clicked() {
                    *env.settings_dirty = true;
                }
            }
        });
        let rtl = Self::rtl(env.book, env.settings);
        if icon(ui, if rtl { Icon::Rtl } else { Icon::Ltr }, false, if rtl { t.rtl } else { t.ltr }).clicked() {
            env.book.rtl = Some(!rtl);
            *env.library_dirty = true;
        }
        if icon(ui, Icon::BlankPage, blank, t.first_blank).clicked() {
            env.book.first_blank = Some(!blank);
            *env.library_dirty = true;
        }
        let disp = match env.settings.display {
            DisplayMode::Single => Icon::Single,
            DisplayMode::Spread => Icon::Spread,
            DisplayMode::Auto => Icon::AutoSpread,
        };
        let r = icon(ui, disp, false, t.display);
        egui::Popup::menu(&r).show(|ui| {
            for (v, l) in [
                (DisplayMode::Single, t.single),
                (DisplayMode::Spread, t.spread),
                (DisplayMode::Auto, t.auto),
            ] {
                if ui.selectable_value(&mut env.settings.display, v, l).clicked() {
                    *env.settings_dirty = true;
                }
            }
        });
        ui.add_space(6.0);
        // page indicator with go-to popup
        let total = self.page_count();
        let label = format!("{} / {}", (page + 1).min(total.max(1)), total);
        let r = ui
            .add(egui::Button::new(egui::RichText::new(label).color(env.pal.muted)).frame(false))
            .on_hover_text(t.go_to_page);
        let mut go: Option<usize> = None;
        egui::Popup::from_toggle_button_response(&r)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(220.0);
                ui.label(t.go_to_page);
                let mut p = page + 1;
                if ui
                    .add(egui::Slider::new(&mut p, 1..=total.max(1)).clamping(egui::SliderClamping::Always))
                    .changed()
                {
                    go = Some(p - 1);
                }
                let te = ui.add(egui::TextEdit::singleline(&mut self.goto_text).desired_width(80.0).hint_text("#"));
                if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Ok(n) = self.goto_text.trim().parse::<usize>() {
                        go = Some(n.clamp(1, total.max(1)) - 1);
                    }
                    self.goto_text.clear();
                }
            });
        if let Some(p) = go {
            self.go_to_page(p, env);
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui, panel: SidePanel, env: &mut Env) {
        let t = env.t;
        ui.horizontal(|ui| {
            let mut p = panel;
            ui.selectable_value(&mut p, SidePanel::Contents, t.contents);
            ui.selectable_value(&mut p, SidePanel::Bookmarks, t.bookmarks);
            if p != panel {
                self.panel = Some(p);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::icons::button(ui, crate::icons::Icon::Close, false, env.t.close).clicked() {
                    self.panel = None;
                }
            });
        });
        ui.separator();
        let mut go: Option<usize> = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match panel {
            SidePanel::Contents => {
                let toc = self.layout.as_ref().map(|l| l.info.toc.clone()).unwrap_or_default();
                if toc.is_empty() {
                    ui.label(egui::RichText::new(t.no_contents).color(env.pal.muted));
                }
                for e in toc {
                    let r = ui.add(egui::Button::new(egui::RichText::new(&e.title)).frame(false).wrap());
                    if r.clicked() {
                        go = Some(e.page);
                    }
                }
            }
            SidePanel::Bookmarks => {
                if env.book.bookmarks.is_empty() {
                    ui.label(egui::RichText::new(t.no_bookmarks).color(env.pal.muted));
                }
                let mut remove = None;
                for (i, b) in env.book.bookmarks.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let p = self.page_of(b.location);
                        if ui.add(egui::Button::new(format!("{} {}", t.page, p + 1)).frame(false)).clicked() {
                            go = Some(p);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::icons::button(ui, crate::icons::Icon::Close, false, env.t.close).clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
                if let Some(i) = remove {
                    env.book.bookmarks.remove(i);
                    *env.library_dirty = true;
                }
            }
        });
        if let Some(p) = go {
            self.go_to_page(p, env);
        }
    }
}
