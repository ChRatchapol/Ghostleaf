//! Dev tool: render pages of a document to PNG.
//! `cargo run -p gl-formats --example render_page -- <file> <out-prefix> [pages...]`
use gl_formats::{LayoutRequest, RenderTheme, engine, open_document};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fonts_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/assets/fonts");
    let fonts: Vec<&'static [u8]> = std::fs::read_dir(fonts_dir)
        .unwrap()
        .flatten()
        .map(|e| &*Box::leak(std::fs::read(e.path()).unwrap().into_boxed_slice()))
        .collect();
    engine::register_bundled_fonts(fonts);
    let mut doc = open_document(std::path::Path::new(&args[1])).expect("open");
    let req = LayoutRequest {
        page_w: 900,
        page_h: 1250,
        scale: 1.5,
        style: Default::default(),
    };
    let t = std::time::Instant::now();
    let info = doc.layout(&req);
    eprintln!(
        "{} pages, toc {:?}, layout {:?}",
        info.page_count,
        info.toc.iter().take(5).collect::<Vec<_>>(),
        t.elapsed()
    );
    for p in args[3..].iter().map(|s| s.parse::<usize>().unwrap()) {
        let img = doc.render(p, 900, 1250, &RenderTheme::default()).unwrap();
        image::save_buffer(
            format!("{}-{p}.png", args[2]),
            &img.pixels,
            img.width,
            img.height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    }
}
