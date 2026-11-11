//! Ghostleaf document formats.
//!
//! Every format implements [`Document`]: it lays itself out for a page size
//! (reflowable formats paginate, fixed formats report page aspect ratios) and
//! renders any page to an RGBA bitmap. The app turns those bitmaps into GPU
//! textures and bends them during the page curl.
//!
//! v1 formats: PDF (PDFium), TXT, CSV/TSV.

pub mod csvdoc;
pub mod decode;
pub mod engine;
pub mod pdf;
pub mod raster;
pub mod thai;
pub mod txt;

pub use gl_core::{Format, Location, TextStyle};
pub use raster::RgbaImage;
use std::path::Path;

/// Colours used when rasterizing reflowable pages (and for PDF dark mode).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTheme {
    pub paper: [u8; 4],
    pub ink: [u8; 4],
    pub muted: [u8; 4],
    pub accent: [u8; 4],
    /// Recolour PDF pages (white -> paper, black -> ink).
    pub recolor_pdf: bool,
}

impl Default for RenderTheme {
    fn default() -> Self {
        Self {
            paper: [255, 255, 255, 255],
            ink: [20, 20, 20, 255],
            muted: [120, 120, 120, 255],
            accent: [60, 60, 60, 255],
            recolor_pdf: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutRequest {
    /// Page size in physical pixels (for reflowable formats).
    pub page_w: u32,
    pub page_h: u32,
    /// Physical pixels per typographic point.
    pub scale: f32,
    pub style: TextStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TocEntry {
    pub title: String,
    pub page: usize,
    pub level: u8,
}

#[derive(Clone, Debug, Default)]
pub struct LayoutInfo {
    pub page_count: usize,
    /// Width / height of each page. Empty for reflowable docs (all pages share the requested size).
    pub aspects: Vec<f32>,
    pub toc: Vec<TocEntry>,
}

pub trait Document {
    fn title(&self) -> String;
    fn format(&self) -> Format;
    fn reflowable(&self) -> bool;
    /// (Re)paginate. Cheap for fixed-layout formats.
    fn layout(&mut self, req: &LayoutRequest) -> LayoutInfo;
    /// Render one page. Fixed formats fit the page into `w x h`; reflowable ones use the layout size.
    fn render(&mut self, page: usize, w: u32, h: u32, theme: &RenderTheme) -> Result<RgbaImage, String>;
    fn location_of(&self, page: usize) -> Location;
    fn page_of(&self, loc: Location) -> usize;
    /// A small cover image, if the format has a natural one.
    fn cover(&mut self, _max_w: u32) -> Option<RgbaImage> {
        None
    }
}

/// Open any supported file. Must be called on the thread that will use the document.
pub fn open_document(path: &Path) -> Result<Box<dyn Document>, String> {
    let format = Format::from_path(path).or_else(|| sniff(path)).ok_or("Unsupported file type")?;
    match format {
        Format::Pdf => Ok(Box::new(pdf::PdfDoc::open(path)?)),
        Format::Txt => Ok(Box::new(txt::TxtDoc::open(path)?)),
        Format::Csv => Ok(Box::new(csvdoc::CsvDoc::open(path)?)),
    }
}

/// Recognise a PDF without the right extension.
fn sniff(path: &Path) -> Option<Format> {
    use std::io::Read;
    let mut b = [0u8; 5];
    std::fs::File::open(path).ok()?.read_exact(&mut b).ok()?;
    (&b == b"%PDF-").then_some(Format::Pdf)
}

/// Title from a file name: "my_great-book.txt" -> "my great book".
pub fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().replace(['_'], " "))
        .unwrap_or_else(|| "Untitled".into())
}
