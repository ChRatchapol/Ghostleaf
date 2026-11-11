//! Shared text engine: one cosmic-text `FontSystem` (bundled + system fonts)
//! and a glyph cache. Used to paginate and paint reflowable pages.

use crate::raster::RgbaImage;
use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, fontdb};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

pub struct TextEngine {
    pub fonts: FontSystem,
    pub cache: SwashCache,
}

static ENGINE: OnceLock<Mutex<TextEngine>> = OnceLock::new();
static BUNDLED: OnceLock<Vec<&'static [u8]>> = OnceLock::new();

/// Register bundled font files. Call once at startup, before any document is opened.
pub fn register_bundled_fonts(fonts: Vec<&'static [u8]>) {
    let _ = BUNDLED.set(fonts);
}

/// Lock the global engine (created lazily; loading system fonts can take ~0.5 s).
pub fn engine() -> MutexGuard<'static, TextEngine> {
    ENGINE
        .get_or_init(|| {
            let sources = BUNDLED
                .get()
                .map(|v| v.iter().map(|b| fontdb::Source::Binary(Arc::new(*b))).collect::<Vec<_>>())
                .unwrap_or_default();
            let mut fonts = FontSystem::new_with_fonts(sources);
            fonts.db_mut().set_serif_family("Literata");
            fonts.db_mut().set_sans_serif_family("Inter");
            Mutex::new(TextEngine {
                fonts,
                cache: SwashCache::new(),
            })
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Font families available for reading (bundled first, then system), de-duplicated and sorted.
pub fn font_families() -> Vec<String> {
    let eng = engine();
    let mut v: Vec<String> = eng
        .fonts
        .db()
        .faces()
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect();
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup();
    v
}

/// Paragraph style handed to the engine.
#[derive(Clone, Debug)]
pub struct ParaStyle<'a> {
    pub family: &'a str,
    pub size_px: f32,
    pub line_px: f32,
    pub bold: bool,
    pub align: Option<Align>,
}

impl TextEngine {
    pub fn layout(&mut self, text: &str, width: f32, height: Option<f32>, st: &ParaStyle) -> Buffer {
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(st.size_px, st.line_px));
        buf.set_size(Some(width.max(1.0)), height);
        let mut attrs = Attrs::new().family(family(st.family));
        if st.bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        buf.set_text(text, &attrs, Shaping::Advanced, st.align);
        buf.shape_until_scroll(&mut self.fonts, false);
        buf
    }

    /// Number of wrapped lines a paragraph occupies.
    pub fn count_lines(&mut self, text: &str, width: f32, st: &ParaStyle) -> usize {
        self.layout(text, width, None, st).layout_runs().count().max(1)
    }

    /// Width of a single unwrapped line of text.
    pub fn measure(&mut self, text: &str, st: &ParaStyle) -> f32 {
        let buf = self.layout(text, f32::MAX / 4.0, None, st);
        buf.layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
    }

    /// Paint lines `from..to` of a laid-out buffer with their top-left at (x, y).
    /// Pixels outside `clip` (x0, y0, x1, y1) are skipped.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        img: &mut RgbaImage,
        buf: &Buffer,
        x: f32,
        y: f32,
        from: usize,
        to: usize,
        color: [u8; 4],
        clip: (i32, i32, i32, i32),
    ) {
        let base = Color::rgba(color[0], color[1], color[2], color[3]);
        let mut top0: Option<f32> = None;
        for (i, run) in buf.layout_runs().enumerate() {
            if i < from {
                continue;
            }
            if i >= to {
                break;
            }
            let t0 = *top0.get_or_insert(run.line_top);
            let oy = y + run.line_y - t0;
            for g in run.glyphs.iter() {
                let pg = g.physical((x, oy), 1.0);
                let c = g.color_opt.unwrap_or(base);
                self.cache.with_pixels(&mut self.fonts, pg.cache_key, c, |gx, gy, col| {
                    let px = pg.x + gx;
                    let py = pg.y + gy;
                    if px >= clip.0 && py >= clip.1 && px < clip.2 && py < clip.3 {
                        img.blend(px, py, [col.r(), col.g(), col.b(), col.a()]);
                    }
                });
            }
        }
    }
}

fn family(name: &str) -> Family<'_> {
    match name {
        "serif" | "Serif" => Family::Serif,
        "sans-serif" | "Sans" => Family::SansSerif,
        "monospace" => Family::Monospace,
        n => Family::Name(n),
    }
}

pub use cosmic_text::Align as TextAlign;

/// Thai text has no glyphs in Latin fonts; pick the matching Thai face
/// (serif reading fonts -> Noto Serif Thai, everything else -> Noto Sans Thai).
/// Both bundled Thai faces also contain Latin letters and digits.
pub fn thai_companion(family: &str) -> &'static str {
    let f = family.to_ascii_lowercase();
    if f.contains("serif") && !f.contains("sans")
        || [
            "literata",
            "georgia",
            "cambria",
            "times",
            "garamond",
            "palatino",
            "book",
            "constantia",
        ]
        .iter()
        .any(|k| f.contains(k))
    {
        "Noto Serif Thai"
    } else {
        "Noto Sans Thai"
    }
}
