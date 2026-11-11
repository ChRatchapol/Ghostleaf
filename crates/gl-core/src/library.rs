use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Pdf,
    Txt,
    Csv,
}

impl Format {
    pub fn from_path(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "pdf" => Some(Format::Pdf),
            "txt" | "text" | "md" | "log" => Some(Format::Txt),
            "csv" | "tsv" | "tab" => Some(Format::Csv),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Format::Pdf => "PDF",
            Format::Txt => "TXT",
            Format::Csv => "CSV",
        }
    }
    pub const EXTENSIONS: &'static [&'static str] = &["pdf", "txt", "text", "md", "log", "csv", "tsv", "tab"];
}

/// A position in a book that survives re-layout (font or window size changes).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// Fixed-layout page index (PDF).
    Page(usize),
    /// Byte offset into the (normalized) text (TXT).
    Text(usize),
    /// Data row index (CSV).
    Row(usize),
}

impl Default for Location {
    fn default() -> Self {
        Location::Page(0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub location: Location,
    pub label: String,
    pub created: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BookEntry {
    /// Content-derived id (BLAKE3 of size + first/last 64 KiB), lets us find a moved file.
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub format: Format,
    pub added: u64,
    pub last_opened: u64,
    #[serde(default)]
    pub location: Location,
    /// 0..1, for the library progress bar.
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
    /// Per-book overrides (None = use the global default).
    #[serde(default)]
    pub first_blank: Option<bool>,
    #[serde(default)]
    pub rtl: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Library {
    #[serde(default)]
    pub books: Vec<BookEntry>,
}

impl Library {
    pub fn get(&self, id: &str) -> Option<&BookEntry> {
        self.books.iter().find(|b| b.id == id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut BookEntry> {
        self.books.iter_mut().find(|b| b.id == id)
    }
    /// Insert or refresh an entry (keeps reading state if the book is already known).
    pub fn upsert(&mut self, id: String, path: PathBuf, title: String, format: Format) -> &mut BookEntry {
        let now = crate::now_secs();
        if let Some(i) = self.books.iter().position(|b| b.id == id) {
            let b = &mut self.books[i];
            b.path = path;
            b.last_opened = now;
            return b;
        }
        self.books.push(BookEntry {
            id,
            path,
            title,
            format,
            added: now,
            last_opened: now,
            location: match format {
                Format::Pdf => Location::Page(0),
                Format::Txt => Location::Text(0),
                Format::Csv => Location::Row(0),
            },
            progress: 0.0,
            bookmarks: Vec::new(),
            first_blank: None,
            rtl: None,
        });
        self.books.last_mut().unwrap()
    }
    pub fn remove(&mut self, id: &str) {
        self.books.retain(|b| b.id != id);
    }
    /// Most recently opened first.
    pub fn sorted_recent(&self) -> Vec<&BookEntry> {
        let mut v: Vec<&BookEntry> = self.books.iter().collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.last_opened));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_keeps_state() {
        let mut lib = Library::default();
        lib.upsert("a".into(), "/x.pdf".into(), "X".into(), Format::Pdf).progress = 0.5;
        let b = lib.upsert("a".into(), "/moved/x.pdf".into(), "X".into(), Format::Pdf);
        assert_eq!(b.progress, 0.5);
        assert_eq!(b.path, PathBuf::from("/moved/x.pdf"));
        assert_eq!(lib.books.len(), 1);
    }

    #[test]
    fn formats() {
        assert_eq!(Format::from_path(Path::new("a.PDF")), Some(Format::Pdf));
        assert_eq!(Format::from_path(Path::new("a.tsv")), Some(Format::Csv));
        assert_eq!(Format::from_path(Path::new("a.epub")), None);
    }
}
