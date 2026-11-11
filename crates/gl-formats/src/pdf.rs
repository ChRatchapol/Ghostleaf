//! PDF via PDFium (the engine used by Chromium). Pages render with the fonts
//! embedded in the PDF. JavaScript/XFA are not part of the PDFium build we ship,
//! and links are never followed automatically.

use crate::{Document, Format, LayoutInfo, LayoutRequest, Location, RenderTheme, RgbaImage, TocEntry};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

/// Folders searched for the PDFium library (pdfium.dll / libpdfium.so / libpdfium.dylib).
pub fn pdfium_search_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        v.push(dir.join("pdfium"));
        v.push(dir.clone());
    }
    if let Ok(cwd) = std::env::current_dir() {
        v.push(cwd);
    }
    // Development builds: the vendored binaries in the repository.
    let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium");
    let plat = if cfg!(windows) {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux-x64"
    };
    v.push(vendor.join(plat));
    v
}

/// Bind PDFium once for the whole process.
pub fn pdfium() -> Result<&'static Pdfium, String> {
    PDFIUM
        .get_or_init(|| {
            for dir in pdfium_search_paths() {
                let lib = Pdfium::pdfium_platform_library_name_at_path(&dir);
                if lib.exists() {
                    match Pdfium::bind_to_library(&lib) {
                        Ok(b) => return Ok(Pdfium::new(b)),
                        Err(e) => return Err(format!("Could not load {}: {e}", lib.display())),
                    }
                }
            }
            Pdfium::bind_to_system_library()
                .map(Pdfium::new)
                .map_err(|_| "PDF engine (pdfium) not found next to the app".to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub struct PdfDoc {
    doc: PdfDocument<'static>,
    title: String,
    aspects: Vec<f32>,
}

impl PdfDoc {
    pub fn open(path: &Path) -> Result<Self, String> {
        let pdfium = pdfium()?;
        let doc = pdfium.load_pdf_from_file(path, None).map_err(|e| match e {
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
                "This PDF is password-protected (not supported yet).".to_string()
            }
            e => format!("Could not open PDF: {e}"),
        })?;
        let title = doc
            .metadata()
            .get(PdfDocumentMetadataTagType::Title)
            .map(|t| t.value().trim().to_string())
            .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("untitled"))
            .unwrap_or_else(|| crate::title_from_path(path));
        let aspects = doc
            .pages()
            .page_sizes()
            .map(|v| {
                v.iter()
                    .map(|r| (r.width().value / r.height().value.max(1.0)).clamp(0.1, 10.0))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self { doc, title, aspects })
    }

    fn toc(&self) -> Vec<TocEntry> {
        self.doc
            .bookmarks()
            .iter()
            .filter_map(|b| {
                let title = b.title()?.trim().to_string();
                let page = b.destination()?.page_index().ok()? as usize;
                (!title.is_empty()).then_some(TocEntry { title, page, level: 0 })
            })
            .take(2000)
            .collect()
    }
}

fn recolor(px: &mut [u8], theme: &RenderTheme) {
    let (p, k) = (theme.paper, theme.ink);
    for c in px.as_chunks_mut::<4>().0 {
        for i in 0..3 {
            let t = 1.0 - c[i] as f32 / 255.0; // 0 = white, 1 = black
            c[i] = (p[i] as f32 + (k[i] as f32 - p[i] as f32) * t).round() as u8;
        }
    }
}

impl Document for PdfDoc {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn format(&self) -> Format {
        Format::Pdf
    }
    fn reflowable(&self) -> bool {
        false
    }

    fn layout(&mut self, _req: &LayoutRequest) -> LayoutInfo {
        LayoutInfo {
            page_count: self.aspects.len(),
            aspects: self.aspects.clone(),
            toc: self.toc(),
        }
    }

    fn render(&mut self, page: usize, w: u32, h: u32, theme: &RenderTheme) -> Result<RgbaImage, String> {
        let p = self.doc.pages().get(page as _).map_err(|e| e.to_string())?;
        let cfg = PdfRenderConfig::new()
            .set_target_width(w.max(1) as _)
            .set_maximum_height(h.max(1) as _)
            .render_form_data(true);
        let bmp = p.render_with_config(&cfg).map_err(|e| e.to_string())?;
        let (bw, bh) = (bmp.width() as u32, bmp.height() as u32);
        let mut px = bmp.as_rgba_bytes();
        if theme.recolor_pdf {
            recolor(&mut px, theme);
        }
        Ok(RgbaImage::from_raw(bw, bh, px))
    }

    fn location_of(&self, page: usize) -> Location {
        Location::Page(page)
    }
    fn page_of(&self, loc: Location) -> usize {
        match loc {
            Location::Page(p) => p.min(self.aspects.len().saturating_sub(1)),
            _ => 0,
        }
    }

    fn cover(&mut self, max_w: u32) -> Option<RgbaImage> {
        let a = *self.aspects.first()?;
        let h = (max_w as f32 / a) as u32;
        self.render(0, max_w, h.max(1), &RenderTheme::default()).ok()
    }
}
