//! One background thread per open book. Owns the document (PDFium handles are
//! thread-bound) and answers layout / render requests. The UI never blocks.

use gl_formats::{Format, LayoutInfo, LayoutRequest, Location, RenderTheme, RgbaImage, open_document};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

pub enum Req {
    Layout {
        id: u64,
        req: LayoutRequest,
    },
    Render {
        layout_id: u64,
        page: usize,
        w: u32,
        h: u32,
        theme: RenderTheme,
        key: u64,
    },
    Cover {
        max_w: u32,
    },
}

pub enum Resp {
    Opened { title: String, format: Format, reflow: bool },
    Layout { id: u64, info: LayoutInfo, starts: Vec<Location> },
    Page { page: usize, key: u64, image: RgbaImage },
    Cover(RgbaImage),
    Error(String),
}

pub struct DocWorker {
    tx: Sender<Req>,
    pub rx: Receiver<Resp>,
}

impl DocWorker {
    pub fn spawn(path: PathBuf, ctx: egui::Context) -> Self {
        let (tx, req_rx) = channel::<Req>();
        let (resp_tx, rx) = channel::<Resp>();
        std::thread::Builder::new()
            .name("ghostleaf-doc".into())
            .spawn(move || run(path, req_rx, resp_tx, ctx))
            .expect("spawn worker");
        Self { tx, rx }
    }

    pub fn send(&self, r: Req) {
        let _ = self.tx.send(r);
    }
}

fn run(path: PathBuf, rx: Receiver<Req>, tx: Sender<Resp>, ctx: egui::Context) {
    let send = |r: Resp| {
        let _ = tx.send(r);
        ctx.request_repaint();
    };
    let mut doc = match open_document(&path) {
        Ok(d) => d,
        Err(e) => {
            send(Resp::Error(e));
            return;
        }
    };
    send(Resp::Opened {
        title: doc.title(),
        format: doc.format(),
        reflow: doc.reflowable(),
    });
    let mut current_layout: u64 = 0;
    let mut queue: Vec<Req> = Vec::new();
    loop {
        if queue.is_empty() {
            match rx.recv() {
                Ok(r) => queue.push(r),
                Err(_) => return, // reader closed
            }
        }
        loop {
            match rx.try_recv() {
                Ok(r) => queue.push(r),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        // Latest layout wins and goes first.
        if let Some(pos) = queue.iter().rposition(|r| matches!(r, Req::Layout { .. })) {
            let Req::Layout { id, req } = queue.remove(pos) else {
                unreachable!()
            };
            queue.retain(|r| !matches!(r, Req::Layout { .. }));
            let info = doc.layout(&req);
            let starts = (0..info.page_count).map(|p| doc.location_of(p)).collect();
            current_layout = id;
            send(Resp::Layout { id, info, starts });
            continue;
        }
        // Then renders (most recently requested first: those are what's on screen now).
        let r = queue.pop().unwrap();
        match r {
            Req::Render {
                layout_id,
                page,
                w,
                h,
                theme,
                key,
            } => {
                if layout_id != current_layout {
                    continue;
                }
                match doc.render(page, w, h, &theme) {
                    Ok(image) => send(Resp::Page { page, key, image }),
                    Err(e) => send(Resp::Error(e)),
                }
            }
            Req::Cover { max_w } => {
                if let Some(img) = doc.cover(max_w) {
                    send(Resp::Cover(img));
                }
            }
            Req::Layout { .. } => unreachable!(),
        }
    }
}
