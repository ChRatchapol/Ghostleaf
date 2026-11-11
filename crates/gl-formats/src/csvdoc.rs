//! CSV / TSV as a paginated table: delimiter and header detection, column
//! widths fitted to the page, header repeated on every page. Tables wider than
//! a page are split into column groups (the first column repeats as a key).

use crate::engine::{ParaStyle, TextAlign, engine};
use crate::raster::mix;
use crate::{Document, Format, LayoutInfo, LayoutRequest, Location, RenderTheme, RgbaImage, decode, thai};
use std::path::Path;

const MAX_CELL_LINES: usize = 4;
const MAX_CELL_CHARS: f32 = 32.0;

pub struct CsvDoc {
    title: String,
    header: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
    ncols: usize,
    layout: Option<Layout>,
}

#[derive(Clone, Debug)]
struct Page {
    rows: std::ops::Range<usize>,
    group: usize,
}

struct Layout {
    req: LayoutRequest,
    groups: Vec<Vec<(usize, f32)>>, // (column index, width px)
    row_lines: Vec<usize>,
    header_lines: usize,
    pages: Vec<Page>,
}

/// Pick the delimiter that splits the first lines most consistently.
pub fn detect_delimiter(sample: &str, is_tsv_ext: bool) -> u8 {
    if is_tsv_ext {
        return b'\t';
    }
    let lines: Vec<&str> = sample.lines().filter(|l| !l.trim().is_empty()).take(30).collect();
    let mut best = (b',', 0.0f32);
    for d in *b",;\t|" {
        let counts: Vec<usize> = lines.iter().map(|l| l.bytes().filter(|&b| b == d).count()).collect();
        if counts.is_empty() || counts[0] == 0 {
            continue;
        }
        let same = counts.iter().filter(|&&c| c == counts[0]).count() as f32 / counts.len() as f32;
        let score = same * 10.0 + (counts[0] as f32).min(20.0) * 0.1;
        if score > best.1 {
            best = (d, score);
        }
    }
    best.0
}

fn looks_numeric(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || ".,-+eE%$ ".contains(c))
}

/// Visible width estimate in "average characters" (combining marks don't count).
fn visible_chars(s: &str) -> f32 {
    s.chars()
        .filter(|&c| !matches!(c, '\u{0300}'..='\u{036F}' | '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}' | '\u{200B}'))
        .count() as f32
}

impl CsvDoc {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let tsv = matches!(
            path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
            Some("tsv" | "tab")
        );
        Self::from_text(crate::title_from_path(path), &decode::decode_text(&bytes), tsv)
    }

    pub fn from_text(title: String, text: &str, tsv: bool) -> Result<Self, String> {
        let sample: String = text.chars().take(20_000).collect();
        let delim = detect_delimiter(&sample, tsv);
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut rows: Vec<Vec<String>> = Vec::new();
        for rec in rdr.records() {
            let rec = rec.map_err(|e| e.to_string())?;
            rows.push(rec.iter().map(|s| s.replace(['\n', '\r'], " ")).collect());
        }
        let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
        // header if the first row has no numeric cells but later rows do, or it's all text and unique
        let header = if rows.len() > 1 {
            let first = &rows[0];
            let first_numeric = first.iter().any(|c| looks_numeric(c));
            let later_numeric = rows.iter().skip(1).take(20).any(|r| r.iter().any(|c| looks_numeric(c)));
            let unique = {
                let mut s: Vec<&String> = first.iter().collect();
                s.sort();
                s.dedup();
                s.len() == first.len()
            };
            (!first_numeric && (later_numeric || unique)).then(|| rows.remove(0))
        } else {
            None
        };
        Ok(Self {
            title,
            header,
            rows,
            ncols,
            layout: None,
        })
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn cell<'a>(&'a self, row: &'a [String], c: usize) -> &'a str {
        row.get(c).map(|s| s.as_str()).unwrap_or("")
    }
}

struct M {
    margin: f32,
    footer: f32,
    content_w: f32,
    content_h: f32,
    font: f32,
    line: f32,
    pad_x: f32,
    pad_y: f32,
}

fn metrics(req: &LayoutRequest) -> M {
    let font = req.style.size * 0.8 * req.scale;
    let margin = (req.style.margin * 0.6) * req.scale;
    let footer = font * 2.2;
    M {
        margin,
        footer,
        content_w: req.page_w as f32 - margin * 2.0,
        content_h: req.page_h as f32 - margin * 2.0 - footer,
        font,
        line: font * 1.35,
        pad_x: font * 0.55,
        pad_y: font * 0.35,
    }
}

fn st<'a>(req: &'a LayoutRequest, m: &M, bold: bool, align: Option<TextAlign>) -> ParaStyle<'a> {
    ParaStyle {
        family: &req.style.font,
        size_px: m.font,
        line_px: m.line,
        bold,
        align,
    }
}

impl Document for CsvDoc {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn format(&self) -> Format {
        Format::Csv
    }
    fn reflowable(&self) -> bool {
        true
    }

    fn layout(&mut self, req: &LayoutRequest) -> LayoutInfo {
        let m = metrics(req);
        let avg = {
            let mut eng = engine();
            let sample = "The quick brown fox jumps over the lazy dog 0123456789";
            eng.measure(sample, &st(req, &m, false, None)) / sample.len() as f32
        }
        .max(1.0);

        // natural column widths from header + a sample of rows
        let mut natural = vec![0.0f32; self.ncols];
        let sample_rows = self.rows.iter().take(400).chain(self.header.iter());
        for r in sample_rows {
            for (c, w) in natural.iter_mut().enumerate() {
                let chars = visible_chars(self.cell(r, c)).min(MAX_CELL_CHARS);
                *w = w.max(chars * avg + m.pad_x * 2.0);
            }
        }
        let min_w = avg * 4.0 + m.pad_x * 2.0;
        for w in natural.iter_mut() {
            *w = w.max(min_w).min(m.content_w * 0.6);
        }

        // greedy column groups; first column repeats as key when splitting
        let total: f32 = natural.iter().sum();
        let mut groups: Vec<Vec<(usize, f32)>> = Vec::new();
        if total <= m.content_w || self.ncols == 1 {
            groups.push((0..self.ncols).map(|c| (c, natural[c])).collect());
        } else {
            let key = natural[0].min(m.content_w * 0.3);
            let mut cur: Vec<(usize, f32)> = vec![(0, key)];
            let mut used = key;
            for c in 1..self.ncols {
                if used + natural[c] > m.content_w && cur.len() > 1 {
                    groups.push(std::mem::replace(&mut cur, vec![(0, key)]));
                    used = key;
                }
                cur.push((c, natural[c]));
                used += natural[c];
            }
            groups.push(cur);
        }
        // stretch each group to the page width
        for g in groups.iter_mut() {
            let sum: f32 = g.iter().map(|x| x.1).sum();
            let k = (m.content_w / sum).max(0.2);
            for x in g.iter_mut() {
                x.1 *= k;
            }
        }

        // estimated wrapped lines per row (max over its cells, using the narrowest width the column gets)
        let col_w: Vec<f32> = (0..self.ncols)
            .map(|c| {
                groups
                    .iter()
                    .flat_map(|g| g.iter())
                    .filter(|x| x.0 == c)
                    .map(|x| x.1)
                    .fold(f32::MAX, f32::min)
            })
            .collect();
        let lines_for = |r: &Vec<String>| -> usize {
            (0..self.ncols)
                .map(|c| {
                    let inner = (col_w[c] - m.pad_x * 2.0).max(avg);
                    ((visible_chars(self.cell(r, c)) * avg / inner).ceil() as usize).clamp(1, MAX_CELL_LINES)
                })
                .max()
                .unwrap_or(1)
        };
        let row_lines: Vec<usize> = self.rows.iter().map(lines_for).collect();
        let header_lines = self.header.as_ref().map(lines_for).unwrap_or(0).min(3);
        let row_h = |n: usize| n as f32 * m.line + m.pad_y * 2.0;
        let header_h = if header_lines > 0 { row_h(header_lines) } else { 0.0 };

        let mut row_pages: Vec<std::ops::Range<usize>> = Vec::new();
        let mut start = 0;
        let mut y = header_h;
        for (i, &n) in row_lines.iter().enumerate() {
            let h = row_h(n);
            if y + h > m.content_h && i > start {
                row_pages.push(start..i);
                start = i;
                y = header_h;
            }
            y += h;
        }
        row_pages.push(start..self.rows.len());

        let mut pages = Vec::new();
        for r in &row_pages {
            for g in 0..groups.len() {
                pages.push(Page { rows: r.clone(), group: g });
            }
        }
        let n = pages.len();
        self.layout = Some(Layout {
            req: req.clone(),
            groups,
            row_lines,
            header_lines,
            pages,
        });
        LayoutInfo {
            page_count: n,
            aspects: Vec::new(),
            toc: Vec::new(),
        }
    }

    fn render(&mut self, page: usize, _w: u32, _h: u32, theme: &RenderTheme) -> Result<RgbaImage, String> {
        let lay = self.layout.as_ref().ok_or("not laid out")?;
        let req = &lay.req;
        let m = metrics(req);
        let mut img = RgbaImage::new(req.page_w, req.page_h, theme.paper);
        let Some(pg) = lay.pages.get(page) else { return Ok(img) };
        let group = &lay.groups[pg.group];
        let grid = mix(theme.paper, theme.ink, 0.14);
        let head_bg = mix(theme.paper, theme.ink, 0.07);
        let zebra = mix(theme.paper, theme.ink, 0.028);
        let row_h = |n: usize| n as f32 * m.line + m.pad_y * 2.0;
        let x0 = m.margin;
        let width: f32 = group.iter().map(|g| g.1).sum();
        let mut y = m.margin;
        let mut eng = engine();

        let draw_row =
            |eng: &mut crate::engine::TextEngine, img: &mut RgbaImage, row: &[String], y: f32, h: f32, bold: bool, color: [u8; 4]| {
                let mut x = x0;
                for &(c, w) in group {
                    let text = thai::insert_word_breaks(row.get(c).map(|s| s.as_str()).unwrap_or(""));
                    let align = if looks_numeric(&text) && !bold {
                        Some(TextAlign::Right)
                    } else {
                        None
                    };
                    let inner_w = (w - m.pad_x * 2.0).max(1.0);
                    let mut style = st(req, &m, bold, align);
                    if thai::contains_thai(&text) {
                        style.family = crate::engine::thai_companion(&req.style.font);
                    }
                    let buf = eng.layout(&text, inner_w, Some(h - m.pad_y * 2.0 + 0.5), &style);
                    let clip = (x as i32, y as i32, (x + w - m.pad_x * 0.5) as i32, (y + h) as i32);
                    eng.draw(img, &buf, x + m.pad_x, y + m.pad_y, 0, MAX_CELL_LINES, color, clip);
                    x += w;
                }
            };

        if let Some(h) = &self.header {
            let hh = row_h(lay.header_lines.max(1));
            img.fill_rect(x0, y, width, hh, head_bg);
            draw_row(&mut eng, &mut img, h, y, hh, true, theme.ink);
            y += hh;
            img.fill_rect(x0, y - 1.0, width, 1.5, mix(theme.paper, theme.ink, 0.4));
        }
        for r in pg.rows.clone() {
            let h = row_h(lay.row_lines[r]);
            if r % 2 == 1 {
                img.fill_rect(x0, y, width, h, zebra);
            }
            draw_row(&mut eng, &mut img, &self.rows[r], y, h, false, theme.ink);
            y += h;
            img.fill_rect(x0, y - 0.5, width, 1.0, grid);
        }
        // vertical rules
        let mut x = x0;
        for (i, &(_, w)) in group.iter().enumerate() {
            if i > 0 {
                img.fill_rect(x - 0.5, m.margin, 1.0, y - m.margin, grid);
            }
            x += w;
        }

        // footer
        let mut label = if pg.rows.is_empty() {
            "—".to_string()
        } else {
            format!("{}–{} / {}", pg.rows.start + 1, pg.rows.end, self.rows.len())
        };
        if lay.groups.len() > 1 {
            label.push_str(&format!("   ·   {}/{}", pg.group + 1, lay.groups.len()));
        }
        let fst = ParaStyle {
            family: &req.style.font,
            size_px: m.font * 0.85,
            line_px: m.font,
            bold: false,
            align: Some(TextAlign::Center),
        };
        let buf = eng.layout(&label, req.page_w as f32, None, &fst);
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
        Location::Row(
            self.layout
                .as_ref()
                .and_then(|l| l.pages.get(page))
                .map(|p| p.rows.start)
                .unwrap_or(0),
        )
    }

    fn page_of(&self, loc: Location) -> usize {
        let row = match loc {
            Location::Row(r) => r,
            _ => 0,
        };
        let Some(l) = &self.layout else { return 0 };
        l.pages
            .iter()
            .position(|p| p.rows.contains(&row) || (p.rows.is_empty() && row == 0))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimiters() {
        assert_eq!(detect_delimiter("a;b;c\n1;2;3\n", false), b';');
        assert_eq!(detect_delimiter("a,b,c\n1,2,3\n", false), b',');
        assert_eq!(detect_delimiter("a\tb\n1\t2\n", false), b'\t');
    }

    #[test]
    fn table_paginates() {
        let mut s = String::from("id,name,city,amount\n");
        for i in 0..300 {
            s.push_str(&format!("{i},Person {i},เชียงใหม่,{}.50\n", i * 3));
        }
        let mut d = CsvDoc::from_text("t".into(), &s, false).unwrap();
        assert!(d.header.is_some());
        assert_eq!(d.row_count(), 300);
        let req = LayoutRequest {
            page_w: 600,
            page_h: 800,
            scale: 1.0,
            style: Default::default(),
        };
        let info = d.layout(&req);
        assert!(info.page_count > 3);
        assert_eq!(d.page_of(d.location_of(2)), 2);
        let img = d.render(0, 0, 0, &RenderTheme::default()).unwrap();
        assert!(img.pixels.chunks(4).any(|p| p[0] < 100));
    }

    #[test]
    fn wide_table_splits_columns() {
        let cols = 30;
        let mut s = (0..cols).map(|c| format!("column_{c}")).collect::<Vec<_>>().join(",") + "\n";
        for i in 0..10 {
            s.push_str(&(0..cols).map(|c| format!("value {i}-{c} long text")).collect::<Vec<_>>().join(","));
            s.push('\n');
        }
        let mut d = CsvDoc::from_text("t".into(), &s, false).unwrap();
        let req = LayoutRequest {
            page_w: 600,
            page_h: 800,
            scale: 1.0,
            style: Default::default(),
        };
        let info = d.layout(&req);
        assert!(info.page_count > 1);
    }
}
