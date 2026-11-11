//! Local storage. One folder, plain JSON, nothing else.
//!
//! Layout:
//! ```text
//! <data dir>/
//!   settings.json     preferences
//!   library.json      known books: path, title, position, bookmarks
//!   covers/<id>.png   optional small PDF cover thumbnails
//! ```
//! Portable mode: if a file named `portable.flag` (or a folder `ghostleaf-data`)
//! exists next to the executable, the data folder is `<exe dir>/ghostleaf-data`.

use crate::{Library, Settings};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

static PLATFORM_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Mobile platforms tell us their app-private data folder (there is no
/// executable folder or XDG/AppData there). Call before [`Store::open`].
pub fn set_platform_root(root: PathBuf) {
    let _ = PLATFORM_ROOT.set(root);
}

#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    portable: bool,
}

#[derive(Clone, Debug)]
pub struct StoredFile {
    pub path: PathBuf,
    pub bytes: u64,
}

impl Store {
    /// Resolve the data folder (portable or per-user) and create it.
    pub fn open() -> Store {
        if let Some(root) = PLATFORM_ROOT.get() {
            return Store::at(root.clone(), false);
        }
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        if let Some(dir) = &exe_dir
            && (dir.join("portable.flag").exists() || dir.join("ghostleaf-data").is_dir())
        {
            return Store::at(dir.join("ghostleaf-data"), true);
        }
        let root = directories::ProjectDirs::from("app", "Ghostleaf", "Ghostleaf")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| exe_dir.unwrap_or_default().join("ghostleaf-data"));
        Store::at(root, false)
    }

    pub fn at(root: PathBuf, portable: bool) -> Store {
        let _ = fs::create_dir_all(&root);
        Store { root, portable }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn is_portable(&self) -> bool {
        self.portable
    }

    fn read_json<T: serde::de::DeserializeOwned + Default>(&self, name: &str) -> T {
        fs::read(self.root.join(name))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_json<T: serde::Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let tmp = self.root.join(format!("{name}.tmp"));
        let data = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
        fs::write(&tmp, data)?;
        fs::rename(&tmp, self.root.join(name))
    }

    pub fn load_settings(&self) -> Settings {
        self.read_json("settings.json")
    }
    pub fn save_settings(&self, s: &Settings) -> io::Result<()> {
        self.write_json("settings.json", s)
    }
    pub fn load_library(&self) -> Library {
        self.read_json("library.json")
    }
    pub fn save_library(&self, l: &Library) -> io::Result<()> {
        self.write_json("library.json", l)
    }

    pub fn cover_path(&self, id: &str) -> PathBuf {
        self.root.join("covers").join(format!("{id}.png"))
    }
    pub fn save_cover(&self, id: &str, png: &[u8]) -> io::Result<()> {
        fs::create_dir_all(self.root.join("covers"))?;
        fs::write(self.cover_path(id), png)
    }
    pub fn delete_cover(&self, id: &str) {
        let _ = fs::remove_file(self.cover_path(id));
    }
    pub fn clear_covers(&self) {
        let _ = fs::remove_dir_all(self.root.join("covers"));
    }

    /// Every file Ghostleaf has stored, for the Privacy screen.
    pub fn list_files(&self) -> Vec<StoredFile> {
        fn walk(dir: &Path, out: &mut Vec<StoredFile>) {
            if let Ok(rd) = fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else if let Ok(m) = e.metadata() {
                        out.push(StoredFile { path: p, bytes: m.len() });
                    }
                }
            }
        }
        let mut v = Vec::new();
        walk(&self.root, &mut v);
        v.sort_by(|a, b| a.path.cmp(&b.path));
        v
    }

    /// Delete everything Ghostleaf stored (settings, library, covers).
    pub fn wipe_all(&self) -> io::Result<()> {
        for f in self.list_files() {
            let _ = fs::remove_file(&f.path);
        }
        let _ = fs::remove_dir_all(self.root.join("covers"));
        Ok(())
    }
}

/// Content-derived book id: BLAKE3 over the file size and its first and last 64 KiB.
/// Fast on huge files, stable when a file is renamed or moved.
/// Open a book file for reading.
///
/// On Android a picked document arrives as a granted descriptor exposed as
/// `/proc/self/fd/N`. Re-opening that path is refused by the storage layer
/// (the app holds no storage permission), so duplicate the descriptor instead.
pub fn open_file(path: &Path) -> io::Result<fs::File> {
    #[cfg(unix)]
    if let Some(fd) = fd_of(path) {
        use std::os::fd::BorrowedFd;
        // SAFETY: the descriptor stays open for as long as the book is in use
        // (it is closed only by the owner of the `/proc/self/fd/N` path).
        let owned = unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?;
        let mut f = fs::File::from(owned);
        f.seek(SeekFrom::Start(0))?;
        return Ok(f);
    }
    fs::File::open(path)
}

/// The descriptor number of a `/proc/self/fd/N` path.
pub fn fd_of(path: &Path) -> Option<i32> {
    path.to_str()?.strip_prefix("/proc/self/fd/")?.parse().ok()
}

pub fn book_id(path: &Path) -> io::Result<String> {
    const CHUNK: u64 = 64 * 1024;
    let mut f = open_file(path)?;
    let len = f.metadata()?.len();
    let mut h = blake3::Hasher::new();
    h.update(&len.to_le_bytes());
    let mut buf = vec![0u8; CHUNK as usize];
    let n = f.read(&mut buf)?;
    h.update(&buf[..n]);
    if len > CHUNK * 2 {
        f.seek(SeekFrom::Start(len - CHUNK))?;
        let n = f.read(&mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex()[..20].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;

    #[test]
    fn roundtrip_and_wipe() {
        let dir = std::env::temp_dir().join(format!("gl-store-test-{}", std::process::id()));
        let store = Store::at(dir.clone(), false);
        let mut s = Settings::default();
        s.sound = false;
        store.save_settings(&s).unwrap();
        assert_eq!(store.load_settings(), s);
        let mut lib = Library::default();
        lib.upsert("abc".into(), "/b.txt".into(), "B".into(), Format::Txt);
        store.save_library(&lib).unwrap();
        store.save_cover("abc", b"png").unwrap();
        assert_eq!(store.load_library(), lib);
        assert_eq!(store.list_files().len(), 3);
        store.wipe_all().unwrap();
        assert!(store.list_files().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn id_is_stable() {
        let p = std::env::temp_dir().join(format!("gl-id-{}.txt", std::process::id()));
        fs::write(&p, "hello world").unwrap();
        let a = book_id(&p).unwrap();
        assert_eq!(a, book_id(&p).unwrap());
        assert_eq!(a.len(), 20);
        let _ = fs::remove_file(p);
    }

    #[cfg(unix)]
    #[test]
    fn reads_through_descriptor_path() {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        let p = std::env::temp_dir().join("gl-fd-test.txt");
        fs::write(&p, b"hello descriptor").unwrap();
        let f = fs::File::open(&p).unwrap();
        let fd_path = PathBuf::from(format!("/proc/self/fd/{}", f.as_raw_fd()));
        assert_eq!(book_id(&fd_path).unwrap(), book_id(&p).unwrap());
        let mut s = String::new();
        open_file(&fd_path).unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "hello descriptor");
        let _ = fs::remove_file(&p);
    }
}
