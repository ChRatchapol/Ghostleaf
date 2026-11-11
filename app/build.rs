// 1. Embed the app icon and version info into ghostleaf.exe on Windows.
//    Needs the Windows SDK resource compiler (installed with the MSVC build tools).
//    If it is missing the build still succeeds, just without the .exe icon.
// 2. Embed recorded page sounds from assets/sounds/{flip,peel,land}-*.wav if present.
//    Those files are licensed for use inside products but not for redistribution,
//    so they are git-ignored; without them the app uses synthesized sounds.
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/ghostleaf.ico");
    println!("cargo:rerun-if-changed=assets/sounds");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/ghostleaf.ico")
            .set("ProductName", "Ghostleaf")
            .set("FileDescription", "Ghostleaf — private offline reader")
            .set("LegalCopyright", "MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Could not embed the Windows icon ({e}); continuing without it.");
        }
    }

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "wav"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::new();
    for kind in ["flip", "peel", "land"] {
        let _ = write!(out, "pub static {}: &[&[u8]] = &[", kind.to_uppercase());
        for f in files.iter().filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{kind}-")))
        }) {
            println!("cargo:rerun-if-changed={}", f.display());
            let _ = write!(out, "include_bytes!(r#\"{}\"#),", f.display());
        }
        out.push_str("];\n");
    }
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("sounds.rs"), out).unwrap();
}
