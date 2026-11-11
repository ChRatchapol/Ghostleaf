//! Turns a `gl_curl::CurlMesh` into egui meshes: the front face, the back face
//! (next page in a spread, or see-through paper in single-page mode) and soft
//! shadows. Shading is baked into vertex colours, so this runs on any backend.

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, TextureId, epaint::Vertex};
use gl_curl::{CurlMesh, V2};

/// Maps the sheet frame (spine at x = 0) to the screen. RTL mirrors horizontally.
#[derive(Clone, Copy, Debug)]
pub struct SheetFrame {
    pub rect: Rect,
    pub rtl: bool,
}

impl SheetFrame {
    pub fn w(&self) -> f32 {
        self.rect.width()
    }
    pub fn h(&self) -> f32 {
        self.rect.height()
    }
    pub fn to_screen(&self, p: V2) -> Pos2 {
        if self.rtl {
            Pos2::new(self.rect.right() - p.x, self.rect.top() + p.y)
        } else {
            Pos2::new(self.rect.left() + p.x, self.rect.top() + p.y)
        }
    }
    pub fn to_local(&self, p: Pos2) -> V2 {
        if self.rtl {
            V2::new(self.rect.right() - p.x, p.y - self.rect.top())
        } else {
            V2::new(p.x - self.rect.left(), p.y - self.rect.top())
        }
    }
    /// Texture u for a sheet-frame u (front face).
    fn front_u(&self, u: f32) -> f32 {
        if self.rtl { 1.0 - u } else { u }
    }
}

pub struct CurlPaint {
    pub front: Option<TextureId>,
    /// Back face texture (spread mode: the page printed on the other side).
    pub back: Option<TextureId>,
    pub paper: Color32,
    pub dark: bool,
}

fn gray(v: f32, a: u8) -> Color32 {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_premultiplied(
        (g as u16 * a as u16 / 255) as u8,
        (g as u16 * a as u16 / 255) as u8,
        (g as u16 * a as u16 / 255) as u8,
        a,
    )
}

fn tinted(paper: Color32, shade: f32) -> Color32 {
    let m = |c: u8| (c as f32 * shade).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(m(paper.r()), m(paper.g()), m(paper.b()))
}

fn mesh_from(
    frame: &SheetFrame,
    cm: &CurlMesh,
    tris: &[[u32; 3]],
    tex: TextureId,
    uv: impl Fn(V2) -> (f32, f32),
    color: impl Fn(&gl_curl::CurlVertex) -> Color32,
) -> Mesh {
    let mut m = Mesh::with_texture(tex);
    // re-index only used vertices
    let mut map = vec![u32::MAX; cm.vertices.len()];
    for t in tris {
        let mut idx = [0u32; 3];
        for (k, &vi) in t.iter().enumerate() {
            if map[vi as usize] == u32::MAX {
                let v = &cm.vertices[vi as usize];
                let (u, w) = uv(v.uv);
                map[vi as usize] = m.vertices.len() as u32;
                m.vertices.push(Vertex {
                    pos: frame.to_screen(v.pos),
                    uv: Pos2::new(u, w),
                    color: color(v),
                });
            }
            idx[k] = map[vi as usize];
        }
        m.indices.extend_from_slice(&idx);
    }
    m
}

/// A soft shadow band: starts at `origin` and runs along `dir` for `len`,
/// spanning `half_span` both ways across. Alpha rises over the first `fade_in`
/// fraction (no hard start) and then falls off exponentially to zero.
fn shadow_band(frame: &SheetFrame, origin: V2, dir: V2, len: f32, half_span: f32, a0: f32, fade_in: f32) -> Shape {
    const N: usize = 12;
    let across = V2::new(-dir.y, dir.x);
    let mut m = Mesh::default();
    for i in 0..=N {
        let t = i as f32 / N as f32;
        let rise = if fade_in > 0.0 { (t / fade_in).min(1.0) } else { 1.0 };
        let rise = rise * rise * (3.0 - 2.0 * rise);
        let a = a0 * rise * (-3.5 * t).exp() * (1.0 - t);
        let c = Color32::from_black_alpha((a.clamp(0.0, 1.0) * 255.0) as u8);
        let mid = origin + dir * (len * t);
        m.colored_vertex(frame.to_screen(mid - across * half_span), c);
        m.colored_vertex(frame.to_screen(mid + across * half_span), c);
    }
    for i in 0..N as u32 {
        let k = i * 2;
        m.add_triangle(k, k + 1, k + 3);
        m.add_triangle(k, k + 3, k + 2);
    }
    Shape::mesh(m)
}

/// Draw shadows that fall on the page *revealed* under the turning sheet.
/// Call after drawing the under-page and before [`draw_sheet`].
pub fn draw_under_shadow(painter: &Painter, frame: &SheetFrame, cm: &CurlMesh, dark: bool) {
    let Some(f) = cm.fold else { return };
    let span = (frame.w() + frame.h()) * 2.0;
    let k = (1.0 - cm.progress).powf(0.7) * (cm.progress * 6.0).min(1.0);
    let strength = if dark { 0.5 } else { 0.28 } * k;
    let width = (frame.w() * 0.16 + f.radius * 3.0).max(12.0);
    // start a little inside the curl's silhouette so the curl hides where the shadow begins
    let origin = f.point + f.normal * (f.radius * 0.55);
    painter.add(shadow_band(frame, origin, f.normal, width, span, strength, 0.12));
}

/// Draw the turning sheet itself.
pub fn draw_sheet(painter: &Painter, frame: &SheetFrame, cm: &CurlMesh, paint: &CurlPaint) {
    // front face
    let front_tex = paint.front.unwrap_or_default();
    let front = mesh_from(
        frame,
        cm,
        &cm.front,
        front_tex,
        |uv| {
            if paint.front.is_some() {
                (frame.front_u(uv.x), uv.y)
            } else {
                (egui::epaint::WHITE_UV.x, egui::epaint::WHITE_UV.y)
            }
        },
        |v| {
            if paint.front.is_some() {
                gray(v.shade(), 255)
            } else {
                tinted(paint.paper, v.shade())
            }
        },
    );
    painter.add(Shape::mesh(front));

    // Shading bands belong to this sheet only: never let them spill onto the facing page.
    let sheet_painter = painter.with_clip_rect(painter.clip_rect().intersect(frame.rect));
    // occlusion just behind the fold, on the still-flat front
    if let Some(f) = cm.fold {
        let span = (frame.w() + frame.h()) * 2.0;
        let a = if paint.dark { 0.22 } else { 0.08 } * (1.0 - cm.progress);
        sheet_painter.add(shadow_band(
            frame,
            f.point,
            f.normal * -1.0,
            (f.radius * 3.0).max(6.0),
            span,
            a,
            0.0,
        ));
    }

    if cm.back.is_empty() {
        return;
    }
    // back face: always a paper base, shaded
    let back_u = |u: f32| if frame.rtl { u } else { 1.0 - u };
    let base = mesh_from(
        frame,
        cm,
        &cm.back,
        TextureId::default(),
        |_| (egui::epaint::WHITE_UV.x, egui::epaint::WHITE_UV.y),
        |v| tinted(paint.paper, v.shade()),
    );
    painter.add(Shape::mesh(base));
    match paint.back {
        Some(tex) => {
            let m = mesh_from(frame, cm, &cm.back, tex, |uv| (back_u(uv.x), uv.y), |v| gray(v.shade(), 255));
            painter.add(Shape::mesh(m));
        }
        None => {
            // see-through paper: the front print, mirrored and faint
            if let Some(tex) = paint.front {
                let a = if paint.dark { 26 } else { 22 };
                let m = mesh_from(frame, cm, &cm.back, tex, |uv| (frame.front_u(uv.x), uv.y), |_| gray(1.0, a));
                painter.add(Shape::mesh(m));
            }
        }
    }
}

/// Soft drop shadow around a page or spread rectangle.
pub fn page_shadow(painter: &Painter, rect: Rect, dark: bool) {
    let s = egui::Shadow {
        offset: [0, 3],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 140 } else { 34 }),
    };
    painter.add(s.as_shape(rect, egui::CornerRadius::same(2)));
}

/// Gutter shading on both sides of the spine (two-page spread).
pub fn gutter(painter: &Painter, spread: Rect, dark: bool) {
    let x = spread.center().x;
    let w = spread.width() * 0.045;
    let a = if dark { 70 } else { 34 };
    for (from, to) in [(x, x - w), (x, x + w)] {
        let mut m = Mesh::default();
        let c0 = Color32::from_black_alpha(a);
        m.colored_vertex(Pos2::new(from, spread.top()), c0);
        m.colored_vertex(Pos2::new(from, spread.bottom()), c0);
        m.colored_vertex(Pos2::new(to, spread.bottom()), Color32::TRANSPARENT);
        m.colored_vertex(Pos2::new(to, spread.top()), Color32::TRANSPARENT);
        m.add_triangle(0, 1, 2);
        m.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(m));
    }
}
