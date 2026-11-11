//! Tiny RGBA canvas: enough to paint text pages and tables.

#[derive(Clone, Debug, PartialEq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA8, row-major.
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, fill: [u8; 4]) -> Self {
        let n = (width as usize) * (height as usize);
        let mut pixels = Vec::with_capacity(n * 4);
        for _ in 0..n {
            pixels.extend_from_slice(&fill);
        }
        Self { width, height, pixels }
    }

    pub fn from_raw(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        debug_assert_eq!(pixels.len(), width as usize * height as usize * 4);
        Self { width, height, pixels }
    }

    /// Alpha-blend a colour onto one pixel (`color[3]` is the coverage/alpha).
    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || color[3] == 0 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let a = color[3] as u32;
        let ia = 255 - a;
        let p = &mut self.pixels[i..i + 4];
        p[0] = ((color[0] as u32 * a + p[0] as u32 * ia + 127) / 255) as u8;
        p[1] = ((color[1] as u32 * a + p[1] as u32 * ia + 127) / 255) as u8;
        p[2] = ((color[2] as u32 * a + p[2] as u32 * ia + 127) / 255) as u8;
        p[3] = (a + p[3] as u32 * ia / 255).min(255) as u8;
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
        let x0 = x.round().max(0.0) as i32;
        let y0 = y.round().max(0.0) as i32;
        let x1 = ((x + w).round() as i32).min(self.width as i32);
        let y1 = ((y + h).round() as i32).min(self.height as i32);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend(xx, yy, color);
            }
        }
    }

    /// Nearest-neighbour-free box downscale (for small cover thumbnails).
    pub fn downscale_to_width(&self, max_w: u32) -> RgbaImage {
        if self.width <= max_w || self.width == 0 {
            return self.clone();
        }
        let f = self.width as f32 / max_w as f32;
        let nw = max_w;
        let nh = ((self.height as f32 / f).round() as u32).max(1);
        let mut out = RgbaImage::new(nw, nh, [0, 0, 0, 0]);
        for y in 0..nh {
            for x in 0..nw {
                let sx0 = (x as f32 * f) as u32;
                let sy0 = (y as f32 * f) as u32;
                let sx1 = (((x + 1) as f32 * f) as u32).clamp(sx0 + 1, self.width);
                let sy1 = (((y + 1) as f32 * f) as u32).clamp(sy0 + 1, self.height);
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                for sy in sy0..sy1 {
                    for sx in sx0..sx1 {
                        let i = ((sy * self.width + sx) * 4) as usize;
                        for c in 0..4 {
                            acc[c] += self.pixels[i + c] as u32;
                        }
                        n += 1;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                for c in 0..4 {
                    out.pixels[o + c] = (acc[c] / n.max(1)) as u8;
                }
            }
        }
        out
    }
}

/// Mix two colours: `t = 0` gives `a`, `t = 1` gives `b`.
pub fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blend_and_fill() {
        let mut img = RgbaImage::new(4, 4, [255, 255, 255, 255]);
        img.fill_rect(0.0, 0.0, 2.0, 2.0, [0, 0, 0, 255]);
        assert_eq!(&img.pixels[0..4], &[0, 0, 0, 255]);
        assert_eq!(&img.pixels[12..16], &[255, 255, 255, 255]);
        let small = img.downscale_to_width(2);
        assert_eq!(small.width, 2);
        assert_eq!(&small.pixels[0..3], &[0, 0, 0]);
    }
}
