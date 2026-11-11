//! Ghostleaf core: settings, library model and local storage.
//!
//! Privacy rule: everything this crate writes is plain, human-readable JSON in a
//! single folder (or next to the executable in portable mode). Book contents are
//! never copied; only the path and a few fields needed to resume reading.

pub mod library;
pub mod settings;
pub mod store;

pub use library::{BookEntry, Bookmark, Format, Library, Location};
pub use settings::*;
pub use store::{Store, StoredFile, book_id};

/// Seconds since the Unix epoch (local clock; used only for sorting "recent").
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
