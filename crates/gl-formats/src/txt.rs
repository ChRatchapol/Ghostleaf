//! Plain-text books: encoding detection, hard-wrap joining, chapter detection,
//! Thai word breaking and pagination into fixed-size pages.

use crate::engine::{ParaStyle, TextAlign, engine};
use crate::{Document, Format, LayoutInfo, LayoutRequest, Location, RenderTheme, RgbaImage, TocEntry, decode, thai};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
struct Para {
    /// Text ready for layout (Thai word breaks inserted).
    text: String,
    /// Byte offset of the paragraph in the normalized source text.
    start: usize,
    /// Source length in bytes (before Thai processing).
    len: usize,
    heading: bool,
}

#[derive(Clone, Debug)]
struct Item {
    para: usize,
    line_from: usize,
    line_to: usize,
    /// Top of the first line, relative to the content box.
    y: f32,
}

#[derive(Clone, Debug)]
struct Page {
    start: usize,
    items: Vec<Item>,
}

struct Layout {
    req: LayoutRequest,
    pages: Vec<Page>,
}

pub struct TxtDoc {
    title: String,
    source: String,
    paras: Vec<Para>,
    joined: Option<bool>,
    layout: Option<Layout>,
}

fn heading_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*((chapter|part|book|prologue|epilogue|preface|introduction)\b.*|บทที่\s*\S+.*|ตอนที่\s*\S+.*|第[^\s]{1,8}[章回節节卷部].*|[IVXLC]+\.\s*$)",
        )
        .unwrap()
    })
}

impl TxtDoc {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Ok(Self::from_text(crate::title_from_path(path), &decode::decode_text(&bytes)))
    }

    pub fn from_text(title: String, text: &str) -> Self {
        let source = decode::normalize_newlines(text);
        Self {
            title,
            source,
            paras: Vec::new(),
            joined: None,
            layout: None,
        }
    }

    /// Hard-wrapped text: most non-empty lines have similar medium length and blocks are separated by blank lines.
    fn looks_hard_wrapped(src: &str) -> bool {
        let lines: Vec<usize> = src.lines().take(4000).map(|l| l.chars().count()).collect();
        let non_empty: Vec<usize> = lines.iter().copied().filter(|&n| n > 0).collect();
        if non_empty.len() < 20 {
            return false;
        }
        let blanks = lines.len() - non_empty.len();
        let mut sorted = non_empty.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        (45..=100).contains(&median) && blanks * 12 >= lines.len()
    }

    fn build_paras(&mut self, join: bool) {
        if self.joined == Some(join) {
            return;
        }
        let join = join && Self::looks_hard_wrapped(&self.source);
        let mut paras = Vec::new();
        let mut offset = 0usize;
        let mut block: Option<(usize, String)> = None;
        let flush = |block: &mut Option<(usize, String)>, paras: &mut Vec<Para>| {
            if let Some((start, text)) = block.take() {
                let trimmed = text.trim_end();
                if !trimmed.trim().is_empty() {
                    let heading = trimmed.chars().count() <= 80 && heading_re().is_match(trimmed);
                    paras.push(Para {
                        text: thai::insert_word_breaks(trimmed),
                        start,
                        len: text.len(),
                        heading,
                    });
                }
            }
        };
        for line in self.source.split('\n') {
            let line_start = offset;
            offset += line.len() + 1;
            if line.trim().is_empty() {
                flush(&mut block, &mut paras);
                continue;
            }
            if join {
                match &mut block {
                    Some((_, t)) if !heading_re().is_match(line) => {
                        t.push(' ');
                        t.push_str(line.trim());
                    }
                    _ => {
                        flush(&mut block, &mut paras);
                        block = Some((line_start, line.trim().to_owned()));
                    }
                }
            } else {
                flush(&mut block, &mut paras);
                block = Some((line_start, line.trim_end().to_owned()));
            }
        }
        flush(&mut block, &mut paras);
        if paras.is_empty() {
            paras.push(Para {
                text: " ".into(),
                start: 0,
                len: 0,
                heading: false,
            });
        }
        self.paras = paras;
        self.joined = Some(join);
        self.layout = None;
    }
}

struct Metrics {
    margin: f32,
    footer: f32,
    content_w: f32,
    content_h: f32,
    body: (f32, f32),
    head: (f32, f32),
    para_gap: f32,
}

fn metrics(req: &LayoutRequest) -> Metrics {
    let s = &req.style;
    let px = s.size * req.scale;
    let margin = s.margin * req.scale;
    let footer = px * 2.0;
    let hs = px * 1.4;
    Metrics {
        margin,
        footer,
        content_w: (req.page_w as f32 - margin * 2.0).max(px * 4.0),
        content_h: (req.page_h as f32 - margin * 2.0 - footer).max(px * 3.0),
        body: (px, px * s.line_height),
        head: (hs, hs * 1.3),
        para_gap: px * s.paragraph_spacing,
    }
}

fn para_style<'a>(req: &'a LayoutRequest, m: &Metrics, heading: bool, thai_text: bool) -> ParaStyle<'a> {
    let (size_px, line_px) = if heading { m.head } else { m.body };
    ParaStyle {
        family: if thai_text {
            crate::engine::thai_companion(&req.style.font)
        } else {
            &req.style.font
        },
        size_px,
        line_px,
        bold: heading,
        align: if heading {
            Some(TextAlign::Left)
        } else if req.style.justify && !thai_text {
            Some(TextAlign::Justified)
        } else {
            None
        },
    }
}

fn para_text(p: &Para, indent: bool) -> std::borrow::Cow<'_, str> {
    if indent && !p.heading {
        format!("\u{2003}\u{2003}{}", p.text).into()
    } else {
        p.text.as_str().into()
    }
}

impl Document for TxtDoc {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn format(&self) -> Format {
        Format::Txt
    }
    fn reflowable(&self) -> bool {
        true
    }

    fn layout(&mut self, req: &LayoutRequest) -> LayoutInfo {
        self.build_paras(req.style.join_wrapped_lines);
        let m = metrics(req);
        let mut pages: Vec<Page> = Vec::new();
        let mut cur = Page {
            start: 0,
            items: Vec::new(),
        };
        let mut y = 0.0f32;
        let mut toc = Vec::new();
        {
            let mut eng = engine();
            for (pi, p) in self.paras.iter().enumerate() {
                let st = para_style(req, &m, p.heading, thai::contains_thai(&p.text));
                let line_h = st.line_px;
                let lines = eng.count_lines(&para_text(p, req.style.indent), m.content_w, &st);
                if p.heading && !cur.items.is_empty() {
                    pages.push(std::mem::replace(
                        &mut cur,
                        Page {
                            start: p.start,
                            items: Vec::new(),
                        },
                    ));
                    y = 0.0;
                }
                if p.heading {
                    y += m.body.1 * 1.2;
                    toc.push(TocEntry {
                        title: p.text.replace(thai::ZWSP, ""),
                        page: pages.len(),
                        level: 0,
                    });
                }
                let mut done = 0;
                while done < lines {
                    let fit = ((m.content_h - y + 0.5) / line_h).floor() as isize;
                    if fit <= 0 {
                        pages.push(std::mem::replace(
                            &mut cur,
                            Page {
                                start: 0,
                                items: Vec::new(),
                            },
                        ));
                        let frac = done as f32 / lines as f32;
                        cur.start = p.start + (p.len as f32 * frac) as usize;
                        y = 0.0;
                        continue;
                    }
                    if cur.items.is_empty() && done == 0 {
                        cur.start = p.start;
                    }
                    let take = (fit as usize).min(lines - done);
                    cur.items.push(Item {
                        para: pi,
                        line_from: done,
                        line_to: done + take,
                        y,
                    });
                    y += take as f32 * line_h;
                    done += take;
                }
                y += if p.heading { m.body.1 * 0.8 } else { m.para_gap };
            }
        }
        if !cur.items.is_empty() || pages.is_empty() {
            pages.push(cur);
        }
        let count = pages.len();
        self.layout = Some(Layout { req: req.clone(), pages });
        LayoutInfo {
            page_count: count,
            aspects: Vec::new(),
            toc,
        }
    }

    fn render(&mut self, page: usize, _w: u32, _h: u32, theme: &RenderTheme) -> Result<RgbaImage, String> {
        let lay = self.layout.as_ref().ok_or("not laid out")?;
        let req = &lay.req;
        let m = metrics(req);
        let mut img = RgbaImage::new(req.page_w, req.page_h, theme.paper);
        let Some(pg) = lay.pages.get(page) else { return Ok(img) };
        let clip = (0, 0, req.page_w as i32, (req.page_h as f32 - m.margin - m.footer * 0.5) as i32);
        let mut eng = engine();
        for it in &pg.items {
            let p = &self.paras[it.para];
            let st = para_style(req, &m, p.heading, thai::contains_thai(&p.text));
            let buf = eng.layout(&para_text(p, req.style.indent), m.content_w, None, &st);
            let color = if p.heading { theme.accent } else { theme.ink };
            eng.draw(&mut img, &buf, m.margin, m.margin + it.y, it.line_from, it.line_to, color, clip);
        }
        // footer: page number
        let st = ParaStyle {
            family: &req.style.font,
            size_px: m.body.0 * 0.72,
            line_px: m.body.0,
            bold: false,
            align: Some(TextAlign::Center),
        };
        let label = format!("{}", page + 1);
        let buf = eng.layout(&label, req.page_w as f32, None, &st);
        let fy = req.page_h as f32 - m.margin - m.footer * 0.6;
        eng.draw(
            &mut img,
            &buf,
            0.0,
            fy,
            0,
            1,
            theme.muted,
            (0, 0, req.page_w as i32, req.page_h as i32),
        );
        Ok(img)
    }

    fn location_of(&self, page: usize) -> Location {
        Location::Text(self.layout.as_ref().and_then(|l| l.pages.get(page)).map(|p| p.start).unwrap_or(0))
    }

    fn page_of(&self, loc: Location) -> usize {
        let off = match loc {
            Location::Text(o) => o,
            _ => 0,
        };
        let Some(l) = &self.layout else { return 0 };
        match l.pages.binary_search_by(|p| p.start.cmp(&off)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(style: crate::TextStyle) -> LayoutRequest {
        LayoutRequest {
            page_w: 600,
            page_h: 800,
            scale: 1.0,
            style,
        }
    }

    #[test]
    fn paginates_and_locates() {
        let mut text = String::new();
        for c in 1..=3 {
            text.push_str(&format!("Chapter {c}\n\n"));
            for p in 0..30 {
                text.push_str(&format!(
                    "Paragraph {p} of chapter {c}. {}\n\n",
                    "Lorem ipsum dolor sit amet. ".repeat(6)
                ));
            }
        }
        let mut d = TxtDoc::from_text("t".into(), &text);
        let info = d.layout(&req(Default::default()));
        assert!(info.page_count > 3, "pages {}", info.page_count);
        assert_eq!(info.toc.len(), 3);
        assert_eq!(info.toc[0].page, 0);
        for i in 0..info.page_count {
            assert_eq!(d.page_of(d.location_of(i)), i);
        }
        let img = d.render(1, 0, 0, &RenderTheme::default()).unwrap();
        assert_eq!((img.width, img.height), (600, 800));
        assert!(img.pixels.chunks(4).any(|p| p[0] < 128), "page should contain ink");
        // re-layout keeps the reading position
        let loc = d.location_of(info.page_count / 2);
        let mut st = crate::TextStyle::default();
        st.size = 22.0;
        let info2 = d.layout(&req(st));
        assert!(info2.page_count > info.page_count);
        let p2 = d.page_of(loc);
        assert!(p2 < info2.page_count);
    }

    #[test]
    fn joins_hard_wrapped() {
        let mut text = String::new();
        for _ in 0..40 {
            text.push_str(
                "This is a hard wrapped line of a plain text book that goes\non and on until the paragraph ends somewhere around here.\n\n",
            );
        }
        let mut d = TxtDoc::from_text("t".into(), &text);
        d.build_paras(true);
        assert_eq!(d.paras.len(), 40);
    }

    #[test]
    fn thai_text_lays_out() {
        let text = "บทที่ 1\n\n".to_string() + &"ภาษาไทยเป็นภาษาที่ไม่มีการเว้นวรรคระหว่างคำ ".repeat(80);
        let mut d = TxtDoc::from_text("t".into(), &text);
        let info = d.layout(&req(Default::default()));
        assert!(info.page_count >= 1);
        assert_eq!(info.toc.len(), 1);
    }
}
