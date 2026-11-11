//! Library: a calm grid of books with covers and reading progress.

use crate::i18n::Tr;
use crate::style::{self, Palette};
use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, TextureHandle, Vec2};
use gl_core::{BookEntry, Format, Library, Store};
use std::collections::HashMap;

pub enum LibAction {
    None,
    Open(String),
    OpenDialog,
    Settings,
    Privacy,
    Remove(String),
}

pub struct Covers {
    map: HashMap<String, Option<TextureHandle>>,
}

impl Covers {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }
    pub fn invalidate(&mut self, id: &str) {
        self.map.remove(id);
    }
    pub fn clear(&mut self) {
        self.map.clear();
    }
    fn get(&mut self, ctx: &egui::Context, store: &Store, id: &str) -> Option<TextureHandle> {
        self.map
            .entry(id.to_owned())
            .or_insert_with(|| {
                let bytes = std::fs::read(store.cover_path(id)).ok()?;
                let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                    .ok()?
                    .to_rgba8();
                let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                Some(ctx.load_texture(format!("cover-{id}"), ci, egui::TextureOptions::LINEAR))
            })
            .clone()
    }
}

const CARD_W: f32 = 148.0;
const COVER_RATIO: f32 = 206.0 / 148.0;

fn cover_tone(title: &str, pal: Palette) -> Color32 {
    let h = title.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let tones: [(u8, u8, u8); 6] = [
        (0xD9, 0xD4, 0xC7),
        (0xC9, 0xD1, 0xCC),
        (0xD6, 0xCC, 0xC4),
        (0xCC, 0xCF, 0xD8),
        (0xD8, 0xD2, 0xBE),
        (0xC6, 0xC6, 0xC2),
    ];
    let (r, g, b) = tones[(h % 6) as usize];
    if pal.dark {
        Color32::from_rgb(r / 4 + 10, g / 4 + 10, b / 4 + 10)
    } else {
        Color32::from_rgb(r, g, b)
    }
}

fn draw_generated_cover(painter: &egui::Painter, rect: Rect, book: &BookEntry, pal: Palette) {
    let tone = cover_tone(&book.title, pal);
    painter.rect_filled(rect, CornerRadius::same(4), tone);
    let ink = if pal.dark { pal.ink } else { Color32::from_rgb(0x22, 0x22, 0x20) };
    let galley = painter.layout(book.title.clone(), style::serif(15.0), ink, rect.width() - 28.0);
    painter.galley(Pos2::new(rect.left() + 14.0, rect.top() + 22.0), galley, ink);
    painter.hline(
        rect.left() + 14.0..=rect.left() + 34.0,
        rect.bottom() - 30.0,
        egui::Stroke::new(1.0, ink.gamma_multiply(0.5)),
    );
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.bottom() - 16.0),
        Align2::LEFT_CENTER,
        book.format.label(),
        egui::FontId::proportional(10.0),
        ink.gamma_multiply(0.7),
    );
}

fn header(ui: &mut egui::Ui, t: &Tr, pal: Palette, act: &mut LibAction, frameless: bool, compact: bool) {
    use crate::icons::{Icon, button as icon};
    ui.horizontal(|ui| {
        let logo = if compact { 24.0 } else { 30.0 };
        let (r, _) = ui.allocate_exact_size(Vec2::splat(logo), Sense::hover());
        style::draw_logo(ui.painter(), r, pal.ink);
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("ghostleaf")
                .font(style::wordmark_font(if compact { 22.0 } else { 28.0 }))
                .color(pal.ink),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if frameless {
                crate::titlebar::window_controls(ui, pal);
                ui.add_space(10.0);
            }
            {
                let open = egui::Button::new(egui::RichText::new(t.open).color(pal.paper).font(style::semibold(14.0)))
                    .fill(pal.ink)
                    .corner_radius(CornerRadius::same(18))
                    .min_size(Vec2::new(0.0, 34.0));
                if ui.add(open).clicked() {
                    *act = LibAction::OpenDialog;
                }
                ui.add_space(6.0);
            }
            if compact {
                if icon(ui, Icon::Settings, false, t.settings).clicked() {
                    *act = LibAction::Settings;
                }
                if icon(ui, Icon::Privacy, false, t.privacy).clicked() {
                    *act = LibAction::Privacy;
                }
            } else {
                if ui.add(egui::Button::new(t.settings).frame(false)).clicked() {
                    *act = LibAction::Settings;
                }
                if ui.add(egui::Button::new(t.privacy).frame(false)).clicked() {
                    *act = LibAction::Privacy;
                }
            }
        });
    });
}

pub fn library_ui(
    ui: &mut egui::Ui,
    lib: &Library,
    store: &Store,
    covers: &mut Covers,
    t: &Tr,
    pal: Palette,
    frameless: bool,
) -> LibAction {
    let mut act = LibAction::None;
    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, 0.0, pal.bg);
    let ctx = ui.ctx().clone();
    if frameless {
        let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), 92.0));
        crate::titlebar::drag_area(ui, bar, egui::Id::unique("library-drag"));
    }
    let compact = style::compact(&ctx);
    let margin = if compact {
        egui::Margin::symmetric(16, 14)
    } else {
        egui::Margin::symmetric(40, 28)
    };
    egui::Frame::new().inner_margin(margin).show(ui, |ui| {
        header(ui, t, pal, &mut act, frameless, compact);

        ui.add_space(26.0);

        if lib.books.is_empty() {
            let avail = ui.available_rect_before_wrap();
            let c = avail.center() - Vec2::new(0.0, 40.0);
            style::draw_logo(
                ui.painter(),
                Rect::from_center_size(c - Vec2::new(0.0, 70.0), Vec2::splat(72.0)),
                pal.line,
            );
            ui.painter().text(
                c + Vec2::new(0.0, 8.0),
                Align2::CENTER_CENTER,
                t.empty_title,
                style::semibold(20.0),
                pal.ink,
            );
            let mut job = egui::text::LayoutJob::simple(t.empty_body.to_string(), egui::FontId::proportional(14.0), pal.muted, 420.0);
            job.halign = egui::Align::Center;
            let g = ui.fonts_mut(|f| f.layout_job(job));
            ui.painter().galley(Pos2::new(c.x, c.y + 30.0), g, pal.muted);
            return;
        }

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = ui.available_width();
            let gap = if compact { 16.0 } else { 28.0 };
            // at least two columns on phones; cards shrink to fit
            let card_w = CARD_W.min(((width - gap) / 2.0).floor()).max(96.0);
            let cover_h = (card_w * COVER_RATIO).round();
            let cols = ((width + gap) / (card_w + gap)).floor().max(1.0) as usize;
            let books = lib.sorted_recent();
            for row in books.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for book in row {
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(card_w, cover_h + 58.0), Sense::click());
                        let cover = Rect::from_min_size(rect.min, Vec2::new(card_w, cover_h));
                        let painter = ui.painter();
                        let missing = !crate::book_available(&book.path);
                        let lift = if resp.hovered() { -3.0 } else { 0.0 };
                        let cover_r = cover.translate(Vec2::new(0.0, lift));
                        let shadow = egui::Shadow {
                            offset: [0, if resp.hovered() { 8 } else { 3 }],
                            blur: if resp.hovered() { 22 } else { 10 },
                            spread: 0,
                            color: Color32::from_black_alpha(if pal.dark { 140 } else { 38 }),
                        };
                        painter.add(shadow.as_shape(cover_r, CornerRadius::same(4)));
                        match (book.format, covers.get(&ctx, store, &book.id)) {
                            (Format::Pdf, Some(tex)) => {
                                painter.rect_filled(cover_r, CornerRadius::same(4), pal.paper);
                                let a = tex.size()[0] as f32 / tex.size()[1].max(1) as f32;
                                let img = crate::reader::fit(a, cover_r);
                                painter.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            _ => draw_generated_cover(painter, cover_r, book, pal),
                        }
                        if missing {
                            painter.rect_filled(cover_r, CornerRadius::same(4), pal.bg.gamma_multiply(0.7));
                        }
                        // progress
                        let bar_y = cover.bottom() + 10.0;
                        painter.hline(cover.left()..=cover.right(), bar_y, egui::Stroke::new(2.0, pal.line));
                        if book.progress > 0.0 {
                            painter.hline(
                                cover.left()..=cover.left() + cover.width() * book.progress.clamp(0.0, 1.0),
                                bar_y,
                                egui::Stroke::new(2.0, pal.ink),
                            );
                        }
                        let tg = painter.layout(book.title.clone(), style::semibold(13.0), pal.ink, card_w);
                        let tg = if tg.rows.len() > 2 {
                            let mut job = egui::text::LayoutJob::simple(book.title.clone(), style::semibold(13.0), pal.ink, card_w);
                            job.wrap.max_rows = 2;
                            ui.fonts_mut(|f| f.layout_job(job))
                        } else {
                            tg
                        };
                        painter.galley(Pos2::new(cover.left(), bar_y + 8.0), tg, pal.ink);
                        if resp.hovered() {
                            let x = Rect::from_center_size(cover_r.right_top() + Vec2::new(-14.0, 14.0), Vec2::splat(22.0));
                            let xr = ui.interact(x, ui.scope_id().with(("rm", &book.id)), Sense::click());
                            painter.circle_filled(x.center(), 11.0, if xr.hovered() { pal.ink } else { pal.panel });
                            crate::icons::draw(
                                painter,
                                x.shrink(3.0),
                                crate::icons::Icon::Close,
                                if xr.hovered() { pal.panel } else { pal.ink },
                            );
                            if xr.clicked() {
                                act = LibAction::Remove(book.id.clone());
                                continue;
                            }
                            xr.on_hover_text(t.remove);
                        }
                        let resp = if missing {
                            resp.on_hover_text(t.file_missing)
                        } else {
                            resp.on_hover_text(book.path.display().to_string())
                        };
                        if resp.clicked() && !missing {
                            act = LibAction::Open(book.id.clone());
                        }
                    }
                });
                ui.add_space(22.0);
            }
        });
    });
    act
}
