//! Visual identity: palettes, fonts and egui styling. Minimal, monochrome.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, epaint::text::FontPriority};
use gl_core::{CustomTheme, Settings, Theme};
use gl_formats::RenderTheme;
use std::sync::Arc;

pub const WORDMARK: &str = "wordmark";
pub const SEMIBOLD: &str = "semibold";

pub static FONT_INTER: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
pub static FONT_INTER_SEMI: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
pub static FONT_LITERATA: &[u8] = include_bytes!("../assets/fonts/Literata-Regular.ttf");
pub static FONT_LITERATA_BOLD: &[u8] = include_bytes!("../assets/fonts/Literata-Bold.ttf");
pub static FONT_THAI_SANS: &[u8] = include_bytes!("../assets/fonts/NotoSansThai-Regular.ttf");
pub static FONT_THAI_SERIF: &[u8] = include_bytes!("../assets/fonts/NotoSerifThai-Regular.ttf");
pub static FONT_THAI_SERIF_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSerifThai-Bold.ttf");
pub static FONT_WORDMARK: &[u8] = include_bytes!("../assets/fonts/MuseoModerno-MediumItalic.ttf");

/// Fonts available to the page renderer (reading fonts; the wordmark font is UI-only).
pub fn reading_fonts() -> Vec<&'static [u8]> {
    vec![
        FONT_LITERATA,
        FONT_LITERATA_BOLD,
        FONT_INTER,
        FONT_INTER_SEMI,
        FONT_THAI_SANS,
        FONT_THAI_SERIF,
        FONT_THAI_SERIF_BOLD,
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub paper: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub dark: bool,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn luminance(c: Color32) -> f32 {
    let l = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * l(c.r()) + 0.7152 * l(c.g()) + 0.0722 * l(c.b())
}

/// Derive a full, readable palette from a paper colour (used by Custom).
pub fn palette_from_paper(paper: Color32, tint: Color32) -> Palette {
    let dark = luminance(paper) < 0.18;
    let black = Color32::from_rgb(0, 0, 0);
    let white = Color32::from_rgb(255, 255, 255);
    let ink = if dark {
        mix(Color32::from_rgb(0xE6, 0xE2, 0xDA), tint, 0.12)
    } else {
        mix(Color32::from_rgb(0x17, 0x15, 0x12), mix(tint, black, 0.7), 0.35)
    };
    let bg = if dark { mix(paper, black, 0.35) } else { mix(paper, black, 0.075) };
    let panel = if dark { mix(paper, white, 0.03) } else { mix(paper, bg, 0.45) };
    Palette {
        bg,
        panel,
        paper,
        ink,
        muted: mix(paper, ink, 0.5),
        line: mix(paper, ink, 0.16),
        dark,
    }
}

pub fn palette_of(theme: Theme, custom: &CustomTheme) -> Palette {
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    match theme {
        Theme::Light => Palette {
            bg: c(0xEC, 0xEA, 0xE5),
            panel: c(0xF6, 0xF5, 0xF2),
            paper: c(0xFF, 0xFF, 0xFD),
            ink: c(0x16, 0x16, 0x16),
            muted: c(0x86, 0x84, 0x80),
            line: c(0xDA, 0xD7, 0xD0),
            dark: false,
        },
        Theme::Paper => Palette {
            bg: c(0xEA, 0xE5, 0xDA),
            panel: c(0xF4, 0xF0, 0xE6),
            paper: c(0xFB, 0xF8, 0xF0),
            ink: c(0x22, 0x1E, 0x18),
            muted: c(0x8C, 0x84, 0x76),
            line: c(0xDD, 0xD5, 0xC6),
            dark: false,
        },
        Theme::Sepia => Palette {
            bg: c(0xE4, 0xD9, 0xC3),
            panel: c(0xEF, 0xE6, 0xD3),
            paper: c(0xF7, 0xEF, 0xDE),
            ink: c(0x3A, 0x2E, 0x21),
            muted: c(0x98, 0x87, 0x6C),
            line: c(0xD5, 0xC6, 0xA8),
            dark: false,
        },
        Theme::Tan => Palette {
            bg: c(0xD6, 0xC3, 0xA0),
            panel: c(0xE2, 0xD1, 0xB0),
            paper: c(0xEB, 0xDC, 0xBC),
            ink: c(0x3B, 0x2A, 0x17),
            muted: c(0x8E, 0x76, 0x52),
            line: c(0xC9, 0xB3, 0x8B),
            dark: false,
        },
        Theme::Dusk => Palette {
            bg: c(0x1A, 0x15, 0x10),
            panel: c(0x24, 0x1D, 0x16),
            paper: c(0x2B, 0x23, 0x1A),
            ink: c(0xE3, 0xD3, 0xB8),
            muted: c(0x9A, 0x88, 0x6C),
            line: c(0x3D, 0x32, 0x26),
            dark: true,
        },
        Theme::Dark => Palette {
            bg: c(0x12, 0x12, 0x12),
            panel: c(0x1B, 0x1B, 0x1B),
            paper: c(0x22, 0x22, 0x21),
            ink: c(0xDD, 0xDA, 0xD3),
            muted: c(0x85, 0x83, 0x7F),
            line: c(0x33, 0x33, 0x32),
            dark: true,
        },
        Theme::Black => Palette {
            bg: c(0x00, 0x00, 0x00),
            panel: c(0x0C, 0x0C, 0x0C),
            paper: c(0x0A, 0x0A, 0x0A),
            ink: c(0xC4, 0xC2, 0xBC),
            muted: c(0x70, 0x6F, 0x6B),
            line: c(0x26, 0x26, 0x26),
            dark: true,
        },
        Theme::Custom => {
            let tint = Color32::from_rgb(custom.tint[0], custom.tint[1], custom.tint[2]);
            let paper = mix(Color32::from_rgb(0xFF, 0xFF, 0xFD), tint, custom.strength);
            palette_from_paper(paper, tint)
        }
    }
}

pub fn palette(s: &Settings) -> Palette {
    palette_of(s.theme, &s.custom_theme)
}

/// Stable key for caches that depend on colours.
pub fn palette_key(p: &Palette) -> u64 {
    [p.paper, p.ink, p.muted, p.bg]
        .iter()
        .fold(17u64, |h, c| h.wrapping_mul(1_000_003) ^ u32::from_le_bytes(c.to_array()) as u64)
}

fn rgba(c: Color32) -> [u8; 4] {
    [c.r(), c.g(), c.b(), 255]
}

pub fn render_theme(s: &Settings) -> RenderTheme {
    let p = palette(s);
    let pdf_dark_invert = s.pdf_dark_invert;
    RenderTheme {
        paper: rgba(p.paper),
        ink: rgba(p.ink),
        muted: rgba(p.muted),
        accent: rgba(p.ink),
        recolor_pdf: p.dark && pdf_dark_invert,
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, data: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(data)));
    };
    add(&mut fonts, "Inter", FONT_INTER);
    add(&mut fonts, "InterSemi", FONT_INTER_SEMI);
    add(&mut fonts, "Literata", FONT_LITERATA);
    add(&mut fonts, "ThaiSans", FONT_THAI_SANS);
    add(&mut fonts, "Wordmark", FONT_WORDMARK);
    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = vec!["Inter".to_owned(), "ThaiSans".to_owned()];
    prop.extend(defaults.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop);
    let mut semi = vec!["InterSemi".to_owned(), "ThaiSans".to_owned()];
    semi.extend(defaults.iter().cloned());
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semi);
    let mut serif = vec!["Literata".to_owned(), "ThaiSans".to_owned()];
    serif.extend(defaults.iter().cloned());
    fonts.families.insert(FontFamily::Name("serif".into()), serif);
    fonts
        .families
        .insert(FontFamily::Name(WORDMARK.into()), vec!["Wordmark".to_owned(), "Inter".to_owned()]);
    let _ = FontPriority::Highest; // (kept for clarity: Inter is first in the list)
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, p: Palette) {
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.bg;
    v.override_text_color = Some(p.ink);
    v.hyperlink_color = p.ink;
    v.selection.bg_fill = p.ink.gamma_multiply(0.16);
    v.selection.stroke = egui::Stroke::new(1.0, p.ink);
    v.window_stroke = egui::Stroke::new(1.0, p.line);
    v.window_corner_radius = egui::CornerRadius::same(14);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.window_shadow = egui::Shadow {
        offset: [0, 10],
        blur: 30,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 120 } else { 40 }),
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 30 }),
    };
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(8);
        w.fg_stroke.color = p.ink;
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.line);
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = p.line.gamma_multiply(0.6);
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = p.ink.gamma_multiply(0.07);
    v.widgets.hovered.bg_fill = p.ink.gamma_multiply(0.10);
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.weak_bg_fill = p.ink.gamma_multiply(0.14);
    v.widgets.active.bg_fill = p.ink.gamma_multiply(0.18);
    v.widgets.active.bg_stroke = egui::Stroke::NONE;
    v.widgets.open.weak_bg_fill = p.ink.gamma_multiply(0.10);
    ctx.set_visuals_of(if p.dark { egui::Theme::Dark } else { egui::Theme::Light }, v);
    ctx.set_theme(if p.dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(11.5));
        s.text_styles
            .insert(egui::TextStyle::Heading, FontId::new(20.0, FontFamily::Name(SEMIBOLD.into())));
    });
}

/// Phone-sized window (or a narrow desktop window): use the compact layout.
pub fn compact(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < 640.0
}

pub fn wordmark_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(WORDMARK.into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn serif(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("serif".into()))
}

/// The Leaf-fold mark, drawn as vector shapes (scales crisply anywhere).
pub fn draw_logo(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    use egui::{Pos2, Shape, Stroke};
    // Geometry from branding/a-leaf-fold.svg, viewBox 128..384.
    let s = rect.width().min(rect.height()) / 256.0;
    let o = rect.center() - egui::vec2(128.0 * s, 128.0 * s);
    let p = |x: f32, y: f32| Pos2::new(o.x + (x - 128.0) * s, o.y + (y - 128.0) * s);
    let mut pts = vec![p(128.0, 384.0)];
    // arc from (128,256) to (256,128): centre (256,256), radius 128
    for i in 0..=16 {
        let a = std::f32::consts::PI + i as f32 / 16.0 * std::f32::consts::FRAC_PI_2;
        pts.push(p(256.0 + 128.0 * a.cos(), 256.0 + 128.0 * a.sin()));
    }
    pts.extend([p(290.0, 128.0), p(290.0, 222.0), p(384.0, 222.0)]);
    for i in 0..=16 {
        let a = i as f32 / 16.0 * std::f32::consts::FRAC_PI_2;
        pts.push(p(256.0 + 128.0 * a.cos(), 256.0 + 128.0 * a.sin()));
    }
    // body is concave at the notch; triangulate as a fan from the centre (star-shaped w.r.t. centre)
    let c = p(240.0, 272.0);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    for q in &pts {
        mesh.colored_vertex(*q, color);
    }
    let n = pts.len() as u32;
    for i in 1..=n {
        let j = if i == n { 1 } else { i + 1 };
        mesh.add_triangle(0, i, j);
    }
    painter.add(Shape::mesh(mesh));
    painter.add(Shape::convex_polygon(
        vec![p(306.0, 128.0), p(384.0, 206.0), p(306.0, 206.0)],
        color,
        Stroke::NONE,
    ));
}
