//! Page-curl geometry and spring physics for Ghostleaf.
//!
//! Renderer-agnostic: produces a deformed vertex grid (2D positions + texture
//! coordinates + lift/angle) that any backend (egui mesh, wgpu, mobile) can draw.
//!
//! Coordinate frame ("sheet frame"): the turning sheet occupies `[0, w] x [0, h]`,
//! its spine (bound edge) is the line `x = 0` and its free edge is `x = w`.
//! `y` grows downwards. Right-to-left books simply mirror this frame when drawing.
//!
//! Model: the part of the sheet beyond a fold line wraps around a cylinder of
//! radius `r` lying on the page, and past half a turn it lies flat, face down.
//! The fold line is placed so the grabbed point lands exactly under the finger.

use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 { self * (1.0 / l) } else { V2::new(1.0, 0.0) }
    }
}
impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}
impl std::ops::Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
}

/// One vertex of the deformed sheet.
#[derive(Clone, Copy, Debug)]
pub struct CurlVertex {
    /// Projected position in the sheet frame (may go to negative x when turned over the spine).
    pub pos: V2,
    /// Texture coordinate of the *front* face, 0..1 (u along x, v along y).
    pub uv: V2,
    /// Height above the page (for depth sorting / shadows).
    pub z: f32,
    /// Wrap angle around the cylinder, 0 (flat, face up) .. PI (flat, face down).
    pub theta: f32,
}

impl CurlVertex {
    /// Simple Lambert-ish brightness for the visible face at this vertex, 0..1.
    pub fn shade(&self) -> f32 {
        let t = self.theta;
        if t <= PI * 0.5 {
            // front face tilting away from the viewer, with a soft highlight
            let base = 0.80 + 0.20 * t.cos();
            let spec = 0.07 * (-((t - 0.85) / 0.3).powi(2)).exp();
            (base + spec).min(1.0)
        } else {
            // back face turning towards the viewer
            0.80 + 0.20 * (-t.cos())
        }
    }
    pub fn is_back(&self) -> bool {
        self.theta > PI * 0.5
    }
}

/// Fold line information, useful for drawing shadows.
#[derive(Clone, Copy, Debug)]
pub struct Fold {
    /// A point on the fold line (where the sheet starts to lift).
    pub point: V2,
    /// Unit normal pointing from the flat part towards the lifted part.
    pub normal: V2,
    /// Cylinder radius.
    pub radius: f32,
}

#[derive(Clone, Debug, Default)]
pub struct CurlMesh {
    pub vertices: Vec<CurlVertex>,
    /// Triangles whose visible face is the front of the sheet, back-to-front order.
    pub front: Vec<[u32; 3]>,
    /// Triangles whose visible face is the back of the sheet, back-to-front order.
    pub back: Vec<[u32; 3]>,
    pub fold: Option<Fold>,
    /// 0 = flat/unturned, 1 = fully turned over the spine.
    pub progress: f32,
}

/// Parameters of a curl.
#[derive(Clone, Copy, Debug)]
pub struct Curl {
    pub w: f32,
    pub h: f32,
    /// The point of the sheet that was grabbed (normally on the free edge `x = w`).
    pub grab: V2,
    /// Where that point is now (the finger / animated target).
    pub drag: V2,
    /// Maximum cylinder radius (in the same units as `w`), e.g. `0.08 * w`.
    pub max_radius: f32,
    /// If true the fold stays vertical (edge grab / keyboard turn).
    pub edge: bool,
}

impl Curl {
    /// Keep the drag point physically reachable so the sheet never tears off the spine.
    pub fn constrained_drag(&self) -> V2 {
        let mut p = self.drag;
        if self.edge {
            p.y = self.grab.y;
            p.x = p.x.clamp(-self.w, self.w);
            return p;
        }
        p.x = p.x.min(self.w);
        // The grabbed point is at distance `w` from the spine point on its own
        // horizontal line, and at most the diagonal from the opposite spine corner.
        let top = self.grab.y < self.h * 0.5;
        let near = V2::new(0.0, if top { 0.0 } else { self.h });
        let far = V2::new(0.0, if top { self.h } else { 0.0 });
        let r_near = (self.grab - near).len();
        let r_far = (self.grab - far).len();
        for _ in 0..3 {
            let d = p - near;
            if d.len() > r_near {
                p = near + d.norm() * r_near;
            }
            let d = p - far;
            if d.len() > r_far {
                p = far + d.norm() * r_far;
            }
        }
        p
    }

    /// Progress of the turn, 0..1, measured along x.
    pub fn progress(&self) -> f32 {
        let p = self.constrained_drag();
        ((self.grab.x - p.x) / (self.grab.x + self.w).max(1e-3)).clamp(0.0, 1.0)
    }

    /// Build the deformed grid with `cols x rows` cells.
    pub fn mesh(&self, cols: u32, rows: u32) -> CurlMesh {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let c = self.grab;
        let p = self.constrained_drag();
        let progress = self.progress();
        let dist = (c - p).len();

        let mut vertices = Vec::with_capacity(((cols + 1) * (rows + 1)) as usize);
        let fold = if dist < 1e-3 {
            None
        } else {
            let n = (c - p).norm();
            // radius grows with the drag and relaxes to 0 at the end so the sheet lands flat
            let ease_out = (1.0 - progress).max(0.0).sqrt();
            let r = (self.max_radius.min(dist / PI) * ease_out).max(0.0);
            let mut m = (c + p) * 0.5 - n * (PI * r * 0.5);
            // The sheet is bound at the spine: the fold may never cross it, so the whole
            // spine edge must stay on the flat side (d <= 0). Otherwise the bound edge
            // would lift and the curl would spill onto the facing page.
            let over = [V2::new(0.0, 0.0), V2::new(0.0, self.h)].map(|q| (q - m).dot(n));
            let over = over[0].max(over[1]);
            if over > 0.0 {
                m = m + n * over;
            }
            Some(Fold {
                point: m,
                normal: n,
                radius: r,
            })
        };

        for j in 0..=rows {
            for i in 0..=cols {
                let u = i as f32 / cols as f32;
                let v = j as f32 / rows as f32;
                let orig = V2::new(u * self.w, v * self.h);
                let mut vx = CurlVertex {
                    pos: orig,
                    uv: V2::new(u, v),
                    z: 0.0,
                    theta: 0.0,
                };
                if let Some(f) = fold {
                    let d = (orig - f.point).dot(f.normal);
                    if d > 1e-3 {
                        let base = orig - f.normal * d;
                        if f.radius > 1e-4 && d < PI * f.radius {
                            let th = d / f.radius;
                            vx.pos = base + f.normal * (f.radius * th.sin());
                            vx.z = f.radius * (1.0 - th.cos());
                            vx.theta = th;
                        } else {
                            let past = d - PI * f.radius;
                            vx.pos = base - f.normal * past;
                            vx.z = 2.0 * f.radius + 1e-3;
                            vx.theta = PI;
                        }
                    }
                }
                vertices.push(vx);
            }
        }

        let idx = |i: u32, j: u32| j * (cols + 1) + i;
        let mut front: Vec<([u32; 3], f32)> = Vec::new();
        let mut back: Vec<([u32; 3], f32)> = Vec::new();
        for j in 0..rows {
            for i in 0..cols {
                let a = idx(i, j);
                let b = idx(i + 1, j);
                let cc = idx(i + 1, j + 1);
                let d = idx(i, j + 1);
                for tri in [[a, b, cc], [a, cc, d]] {
                    let vs = tri.map(|k| vertices[k as usize]);
                    let th = (vs[0].theta + vs[1].theta + vs[2].theta) / 3.0;
                    let z = (vs[0].z + vs[1].z + vs[2].z) / 3.0;
                    if th > PI * 0.5 { back.push((tri, z)) } else { front.push((tri, z)) }
                }
            }
        }
        front.sort_by(|a, b| a.1.total_cmp(&b.1));
        back.sort_by(|a, b| a.1.total_cmp(&b.1));
        CurlMesh {
            vertices,
            front: front.into_iter().map(|t| t.0).collect(),
            back: back.into_iter().map(|t| t.0).collect(),
            fold,
            progress,
        }
    }
}

/// A critically damped 2D spring (frame-rate independent).
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub pos: V2,
    pub vel: V2,
    pub target: V2,
    /// Angular frequency; higher = snappier. ~12-20 feels like paper.
    pub omega: f32,
}

impl Spring {
    pub fn new(pos: V2, omega: f32) -> Self {
        Self {
            pos,
            vel: V2::default(),
            target: pos,
            omega,
        }
    }
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let w = self.omega;
        let x = self.pos - self.target;
        let e = (-w * dt).exp();
        let tmp = self.vel + x * w;
        let nx = (x + tmp * dt) * e;
        let nv = (self.vel - tmp * (w * dt)) * e;
        self.pos = self.target + nx;
        self.vel = nv;
    }
    pub fn settled(&self, eps: f32) -> bool {
        (self.pos - self.target).len() < eps && self.vel.len() < eps * 8.0
    }
}

/// Velocity tracker for flings (exponentially smoothed).
#[derive(Clone, Copy, Debug, Default)]
pub struct Velocity {
    pub v: V2,
    last: Option<(V2, f64)>,
}
impl Velocity {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn push(&mut self, p: V2, t: f64) {
        if let Some((lp, lt)) = self.last {
            let dt = (t - lt) as f32;
            if dt > 1e-4 {
                let inst = (p - lp) * (1.0 / dt);
                self.v = self.v * 0.6 + inst * 0.4;
            }
        }
        self.last = Some((p, t));
    }
}

/// Decide if a released drag should complete the turn.
pub fn should_complete(progress: f32, velocity_x: f32, w: f32) -> bool {
    let fling = -velocity_x / w.max(1.0); // sheet widths per second, positive = towards spine
    if fling > 1.2 {
        return true;
    }
    if fling < -1.2 {
        return false;
    }
    progress > 0.35
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curl(drag: V2) -> Curl {
        Curl {
            w: 100.0,
            h: 150.0,
            grab: V2::new(100.0, 150.0),
            drag,
            max_radius: 8.0,
            edge: false,
        }
    }

    #[test]
    fn flat_when_not_dragged() {
        let m = curl(V2::new(100.0, 150.0)).mesh(8, 8);
        assert!(m.fold.is_none());
        assert!(m.back.is_empty());
        assert!(m.vertices.iter().all(|v| v.theta == 0.0));
    }

    #[test]
    fn grabbed_corner_follows_finger() {
        let target = V2::new(40.0, 120.0);
        let c = curl(target);
        let m = c.mesh(20, 30);
        let corner = m.vertices.last().unwrap();
        let p = c.constrained_drag();
        assert!((corner.pos - p).len() < 1.5, "corner {:?} vs {:?}", corner.pos, p);
        assert!(corner.is_back());
    }

    #[test]
    fn spine_never_moves() {
        let m = curl(V2::new(-60.0, 100.0)).mesh(10, 10);
        for v in m.vertices.iter().filter(|v| v.uv.x == 0.0) {
            assert!(v.pos.x.abs() < 1e-3 || v.theta > 0.0);
        }
    }

    #[test]
    fn fully_turned_is_mirror() {
        let c = Curl {
            edge: true,
            ..curl(V2::new(-100.0, 150.0))
        };
        let m = c.mesh(10, 1);
        assert!((c.progress() - 1.0).abs() < 1e-4);
        for v in &m.vertices {
            assert!((v.pos.x + v.uv.x * 100.0).abs() < 1e-2, "{:?}", v);
        }
    }

    #[test]
    fn spine_edge_stays_flat_for_any_drag() {
        // includes "drag a top corner straight down", which used to lift the spine
        let drags = [
            V2::new(100.0, 140.0),
            V2::new(90.0, 60.0),
            V2::new(20.0, 10.0),
            V2::new(-50.0, 75.0),
            V2::new(60.0, -40.0),
        ];
        for grab in [V2::new(100.0, 0.0), V2::new(100.0, 150.0)] {
            for d in drags {
                let c = Curl {
                    w: 100.0,
                    h: 150.0,
                    grab,
                    drag: d,
                    max_radius: 9.0,
                    edge: false,
                };
                let m = c.mesh(20, 30);
                for v in m.vertices.iter().filter(|v| v.uv.x == 0.0) {
                    assert!(v.theta == 0.0 && v.z == 0.0, "spine lifted: grab {grab:?} drag {d:?} -> {v:?}");
                }
            }
        }
    }

    #[test]
    fn drag_is_constrained() {
        let c = curl(V2::new(-500.0, -500.0));
        let p = c.constrained_drag();
        assert!((p - V2::new(0.0, 150.0)).len() <= 100.0 + 1e-3);
    }

    #[test]
    fn spring_settles() {
        let mut s = Spring::new(V2::new(0.0, 0.0), 16.0);
        s.target = V2::new(100.0, 50.0);
        for _ in 0..200 {
            s.step(1.0 / 60.0);
        }
        assert!(s.settled(0.5));
    }

    #[test]
    fn completion_rules() {
        assert!(should_complete(0.5, 0.0, 100.0));
        assert!(!should_complete(0.1, 0.0, 100.0));
        assert!(should_complete(0.1, -500.0, 100.0));
        assert!(!should_complete(0.8, 500.0, 100.0));
    }
}
