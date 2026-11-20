//! Software rendering surface and primitive drawing.
//!
//! Everything is drawn into a 32-bit 0x00RRGGBB backbuffer and presented to the
//! GOP framebuffer with a single conversion pass. Anti-aliasing uses analytic
//! coverage (rounded rectangles, circles) or the font atlas alpha channel.

use alloc::vec;
use alloc::vec::Vec;

use crate::font_data;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    pub fn inset(&self, amount: i32) -> Self {
        Self::new(
            self.x + amount,
            self.y + amount,
            (self.w - amount * 2).max(0),
            (self.h - amount * 2).max(0),
        )
    }

    pub fn intersect(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Rect::new(x, y, (right - x).max(0), (bottom - y).max(0))
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Color(pub u32);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    pub const fn from_hex(hex: u32) -> Self {
        Color(0xFF00_0000 | (hex & 0x00FF_FFFF))
    }

    pub fn alpha(self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }

    pub fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub fn b(self) -> u8 {
        self.0 as u8
    }

    pub fn opaque(self) -> Self {
        Color(self.0 | 0xFF00_0000)
    }

    pub fn with_alpha(self, alpha: u8) -> Self {
        Color((self.0 & 0x00FF_FFFF) | ((alpha as u32) << 24))
    }

    pub fn scale_alpha(self, factor: f32) -> Self {
        let a = ((self.alpha() as f32) * factor.clamp(0.0, 1.0)) as u8;
        self.with_alpha(a)
    }

    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        Color::rgba(
            mix(self.r(), other.r()),
            mix(self.g(), other.g()),
            mix(self.b(), other.b()),
            mix(self.alpha(), other.alpha()),
        )
    }

    pub fn lerp(self, other: Color, t: f32) -> Self {
        self.mix(other, t)
    }
}

#[inline]
fn blend(dst: u32, src: Color) -> u32 {
    let a = src.alpha() as u32;
    if a == 255 {
        return src.0 & 0x00FF_FFFF;
    }
    if a == 0 {
        return dst;
    }
    let inv = 255 - a;
    let dr = (dst >> 16) & 0xFF;
    let dg = (dst >> 8) & 0xFF;
    let db = dst & 0xFF;
    let r = (src.r() as u32 * a + dr * inv) / 255;
    let g = (src.g() as u32 * a + dg * inv) / 255;
    let b = (src.b() as u32 * a + db * inv) / 255;
    (r << 16) | (g << 8) | b
}

pub struct Surface {
    pub width: i32,
    pub height: i32,
    pub pixels: *mut u32,
    pub clip: Rect,
    pub format: u32,
}

impl Surface {
    pub fn new(width: i32, height: i32, format: u32) -> Option<Self> {
        let bytes = width as usize * height as usize * 4;
        let address = crate::sys::vm_alloc(bytes)?;
        Some(Self {
            width,
            height,
            pixels: address as *mut u32,
            clip: Rect::new(0, 0, width, height),
            format,
        })
    }

    pub fn set_clip(&mut self, clip: Rect) -> Rect {
        let previous = self.clip;
        self.clip = clip.intersect(Rect::new(0, 0, self.width, self.height));
        previous
    }

    /// Raw pixel read, ignoring the clip. Used to save the area behind a
    /// window's rounded corners before painting it.
    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return 0;
        }
        unsafe { *self.pixels.add((y * self.width + x) as usize) }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, value: u32) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        unsafe {
            *self.pixels.add((y * self.width + x) as usize) = value;
        }
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, color: Color) {
        if !self.clip.contains(x, y) {
            return;
        }
        let index = (y * self.width + x) as usize;
        unsafe {
            let pixel = self.pixels.add(index);
            *pixel = blend(*pixel, color);
        }
    }

    pub fn clear(&mut self, color: Color) {
        let value = color.opaque().0 & 0x00FF_FFFF;
        let count = self.width as usize * self.height as usize;
        unsafe {
            for i in 0..count {
                *self.pixels.add(i) = value;
            }
        }
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rect = rect.intersect(self.clip);
        if rect.is_empty() {
            return;
        }
        if color.alpha() == 255 {
            let value = color.0 & 0x00FF_FFFF;
            for y in rect.y..rect.bottom() {
                let start = (y * self.width + rect.x) as usize;
                let end = (y * self.width + rect.right()) as usize;
                unsafe {
                    for i in start..end {
                        *self.pixels.add(i) = value;
                    }
                }
            }
            return;
        }
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                self.put(x, y, color);
            }
        }
    }

    pub fn vertical_gradient(&mut self, rect: Rect, top: Color, bottom: Color) {
        let rect = rect.intersect(self.clip);
        if rect.is_empty() {
            return;
        }
        for y in rect.y..rect.bottom() {
            let t = if rect.h > 1 {
                (y - rect.y) as f32 / (rect.h - 1) as f32
            } else {
                0.0
            };
            let color = top.lerp(bottom, t);
            for x in rect.x..rect.right() {
                self.put(x, y, color);
            }
        }
    }

    fn span(&mut self, y: i32, from: f32, to: f32, color: Color) {
        if to <= from {
            return;
        }
        let left = from.max(self.clip.x as f32);
        let right = to.min(self.clip.right() as f32);
        if right <= left {
            return;
        }
        let first = left.ceil() as i32;
        let last = right.floor() as i32;
        // Partial coverage on both ends.
        let left_coverage = (first as f32 - left).clamp(0.0, 1.0);
        if left_coverage > 0.0 {
            self.put(first - 1, y, color.scale_alpha(left_coverage));
        }
        let right_coverage = (right - last as f32).clamp(0.0, 1.0);
        for x in first..last {
            self.put(x, y, color);
        }
        if right_coverage > 0.0 {
            self.put(last, y, color.scale_alpha(right_coverage));
        }
    }

    /// Anti-aliased rounded rectangle. O(height) square roots; interior spans
    /// are filled with straight stores, keeping the common case fast.
    pub fn fill_rounded(&mut self, rect: Rect, radius: i32, color: Color) {
        if rect.is_empty() {
            return;
        }
        let radius = radius.min(rect.w / 2).min(rect.h / 2).max(0);
        if radius == 0 {
            self.fill_rect(rect, color);
            return;
        }
        let top = rect.y.max(self.clip.y);
        let bottom = rect.bottom().min(self.clip.bottom());
        let r = radius as f32;
        for y in top..bottom {
            let dy = if y < rect.y + radius {
                (rect.y + radius - y) as f32
            } else if y >= rect.bottom() - radius {
                (y - (rect.bottom() - radius - 1)) as f32
            } else {
                0.0
            };
            let inset = if dy > 0.0 {
                r - (r * r - dy * dy).max(0.0).sqrt()
            } else {
                0.0
            };
            self.span(
                y,
                rect.x as f32 + inset,
                rect.right() as f32 - inset,
                color,
            );
        }
    }

    pub fn stroke_rounded(&mut self, rect: Rect, radius: i32, thickness: i32, color: Color) {
        for i in 0..thickness.max(1) {
            let r = rect.inset(i);
            if r.is_empty() {
                break;
            }
            // Draw the ring by filling outer then punching inner is not
            // possible without blending; use horizontal/vertical edge lines
            // plus corner arcs.
            let radius = (radius - i).max(0);
            self.fill_rect(Rect::new(r.x + radius, r.y, r.w - radius * 2, 1), color);
            self.fill_rect(
                Rect::new(r.x + radius, r.bottom() - 1, r.w - radius * 2, 1),
                color,
            );
            self.fill_rect(Rect::new(r.x, r.y + radius, 1, r.h - radius * 2), color);
            self.fill_rect(
                Rect::new(r.right() - 1, r.y + radius, 1, r.h - radius * 2),
                color,
            );
            // Corner arcs via circle outline quadrants.
            self.arc(r.x + radius, r.y + radius, radius, core::f32::consts::PI, core::f32::consts::PI * 1.5, color);
            self.arc(r.right() - radius - 1, r.y + radius, radius, core::f32::consts::PI * 1.5, core::f32::consts::PI * 2.0, color);
            self.arc(r.right() - radius - 1, r.bottom() - radius - 1, radius, 0.0, core::f32::consts::PI * 0.5, color);
            self.arc(r.x + radius, r.bottom() - radius - 1, radius, core::f32::consts::PI * 0.5, core::f32::consts::PI, color);
        }
    }

    fn arc(&mut self, cx: i32, cy: i32, radius: i32, start: f32, end: f32, color: Color) {
        if radius <= 0 {
            return;
        }
        let r = radius as f32;
        let steps = (radius * 2).max(4);
        let mut previous: Option<(i32, i32)> = None;
        for step in 0..=steps {
            let angle = start + (end - start) * (step as f32 / steps as f32);
            let point = (
                cx + (angle.cos() * r).round() as i32,
                cy + (angle.sin() * r).round() as i32,
            );
            if let Some((px, py)) = previous {
                self.line(px, py, point.0, point.1, color);
            }
            previous = Some(point);
        }
    }

    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        if radius <= 0 {
            return;
        }
        let top = (cy - radius).max(self.clip.y);
        let bottom = (cy + radius + 1).min(self.clip.bottom());
        let r = radius as f32;
        for y in top..bottom {
            let dy = (y as f32 + 0.5) - cy as f32;
            let inside = (r * r - dy * dy).max(0.0).sqrt();
            self.span(y, cx as f32 - inside, cx as f32 + inside, color);
        }
    }

    pub fn stroke_circle(&mut self, cx: i32, cy: i32, radius: i32, thickness: i32, color: Color) {
        for i in 0..thickness {
            let r = radius - i;
            if r <= 0 {
                break;
            }
            let mut previous: Option<(i32, i32)> = None;
            for step in 0..=360 {
                let angle = step as f32 * core::f32::consts::PI / 180.0;
                let x = cx + (angle.cos() * r as f32).round() as i32;
                let y = cy + (angle.sin() * r as f32).round() as i32;
                if let Some((px, py)) = previous {
                    self.line(px, py, x, y, color);
                }
                previous = Some((x, y));
            }
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            self.put(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = error * 2;
            if e2 >= dy {
                error += dy;
                x += sx;
            }
            if e2 <= dx {
                error += dx;
                y += sy;
            }
        }
    }

    pub fn rounded_shadow(&mut self, shadow: &[u8], x: i32, y: i32, width: i32, height: i32) {
        let mut sy = 0;
        while sy < height {
            let dy = y + sy;
            if dy < self.clip.y || dy >= self.clip.bottom() {
                sy += 1;
                continue;
            }
            let mut sx = 0;
            while sx < width {
                let dx = x + sx;
                if dx >= self.clip.x && dx < self.clip.right() {
                    let alpha = shadow[(sy * width + sx) as usize];
                    if alpha > 0 {
                        let index = (dy * self.width + dx) as usize;
                        unsafe {
                            let pixel = self.pixels.add(index);
                            let value = *pixel;
                            let inv = 255u32 - alpha as u32;
                            let r = ((value >> 16) & 0xFF) * inv / 255;
                            let g = ((value >> 8) & 0xFF) * inv / 255;
                            let b = (value & 0xFF) * inv / 255;
                            *pixel = (r << 16) | (g << 8) | b;
                        }
                    }
                }
                sx += 1;
            }
            sy += 1;
        }
    }

    /// Translucent panel sampled from a pre-blurred backdrop. Integer math
    /// only: this runs for tens of thousands of pixels per frame and QEMU's
    /// software floating point is several times slower than integer blends.
    pub fn glass(&mut self, rect: Rect, radius: i32, tint: Color, backdrop: &[u32], strength: f32) {
        let radius = radius.min(rect.w / 2).min(rect.h / 2).max(0);
        let alpha = ((tint.alpha() as f32) * strength).clamp(0.0, 255.0) as u32;
        if alpha == 0 {
            return;
        }
        let inverse = 255 - alpha;
        let tint_r = tint.r() as u32;
        let tint_g = tint.g() as u32;
        let tint_b = tint.b() as u32;
        let r = radius as f32;
        let mut previous = self.clip;
        self.clip = self.clip.intersect(rect);
        for y in rect.y..rect.bottom() {
            let dy = if y < rect.y + radius {
                (rect.y + radius - y) as f32
            } else if y >= rect.bottom() - radius {
                (y - (rect.bottom() - radius - 1)) as f32
            } else {
                0.0
            };
            let inset = if dy > 0.0 {
                r - (r * r - dy * dy).max(0.0).sqrt()
            } else {
                0.0
            };
            let from = rect.x as f32 + inset;
            let to = rect.right() as f32 - inset;
            let first = from.ceil() as i32;
            let last = to.floor() as i32;
            for x in first..last {
                if !self.clip.contains(x, y) {
                    continue;
                }
                let sample = if x >= 0 && y >= 0 && x < self.width && y < self.height {
                    let index = (y * self.width + x) as usize;
                    if index < backdrop.len() {
                        backdrop[index]
                    } else {
                        0
                    }
                } else {
                    0
                };
                let sr = (sample >> 16) & 0xFF;
                let sg = (sample >> 8) & 0xFF;
                let sb = sample & 0xFF;
                let rr = (sr * inverse + tint_r * alpha) / 255;
                let gg = (sg * inverse + tint_g * alpha) / 255;
                let bb = (sb * inverse + tint_b * alpha) / 255;
                let index = (y * self.width + x) as usize;
                unsafe {
                    *self.pixels.add(index) = (rr << 16) | (gg << 8) | bb;
                }
            }
        }
        self.clip = previous;
        self.stroke_rounded(
            rect,
            radius,
            1,
            Color::rgba(255, 255, 255, if strength > 0.5 { 26 } else { 10 }),
        );
    }

    pub fn hline(&mut self, rect: Rect, color: Color) {
        self.fill_rect(Rect::new(rect.x, rect.y, rect.w, 1), color);
    }

    pub fn text_width(&self, text: &str, face: usize) -> i32 {
        let face = &font_data::FACES[face];
        let mut width = 0i32;
        for ch in text.bytes() {
            if ch < font_data::FIRST as u8 || ch > font_data::LAST as u8 {
                continue;
            }
            let index = face.first as usize + (ch as usize - font_data::FIRST as usize);
            width += font_data::GLYPHS[index].advance as i32;
        }
        width
    }

    pub fn draw_text(&mut self, x: i32, top: i32, text: &str, face_index: usize, color: Color) -> i32 {
        let face = font_data::FACES[face_index];
        let baseline = top + face.baseline as i32;
        let mut pen = x;
        for ch in text.bytes() {
            if ch == b'\n' {
                continue;
            }
            let ch = if ch < font_data::FIRST as u8 || ch > font_data::LAST as u8 {
                b'?'
            } else {
                ch
            };
            let index = face.first as usize + (ch as usize - font_data::FIRST as usize);
            let glyph = font_data::GLYPHS[index];
            if glyph.w > 0 && glyph.h > 0 {
                let gx = pen + glyph.xoff as i32;
                let gy = baseline + glyph.yoff as i32;
                let offset = glyph.offset as usize;
                for row in 0..glyph.h as i32 {
                    let py = gy + row;
                    if py < self.clip.y || py >= self.clip.bottom() {
                        continue;
                    }
                    for col in 0..glyph.w as i32 {
                        let px = gx + col;
                        if px < self.clip.x || px >= self.clip.right() {
                            continue;
                        }
                        let coverage = font_data::ATLAS[offset + row as usize * glyph.w as usize + col as usize];
                        if coverage == 0 {
                            continue;
                        }
                        let alpha = (coverage as u32 * color.alpha() as u32 / 255) as u8;
                        if alpha == 0 {
                            continue;
                        }
                        let index = (py * self.width + px) as usize;
                        unsafe {
                            let pixel = self.pixels.add(index);
                            *pixel = blend(*pixel, color.with_alpha(alpha));
                        }
                    }
                }
            }
            pen += glyph.advance as i32;
        }
        pen - x
    }

    pub fn draw_text_centered(&mut self, rect: Rect, top: i32, text: &str, face: usize, color: Color) {
        let width = self.text_width(text, face);
        self.draw_text(rect.x + (rect.w - width) / 2, top, text, face, color);
    }

    pub fn present(&self, framebuffer: *mut u32, fb_stride: u32, format: u32) {
        let count = self.width as usize * self.height as usize;
        unsafe {
            if format == 1 {
                core::ptr::copy_nonoverlapping(self.pixels, framebuffer, count);
            } else {
                for y in 0..self.height {
                    let row = (y as u32 * fb_stride) as usize;
                    for x in 0..self.width {
                        let c = *self.pixels.add((y * self.width + x) as usize);
                        let b = c & 0xFF;
                        let g = (c >> 8) & 0xFF;
                        let r = (c >> 16) & 0xFF;
                        *framebuffer.add(row + x as usize) = (b << 16) | (g << 8) | r;
                    }
                }
            }
        }
    }

    pub fn fill_rounded_gradient(&mut self, rect: Rect, radius: i32, top: Color, bottom: Color) {
        if rect.is_empty() {
            return;
        }
        let radius = radius.min(rect.w / 2).min(rect.h / 2).max(0);
        if radius == 0 {
            self.vertical_gradient(rect, top, bottom);
            return;
        }
        let r = radius as f32;
        let top_y = rect.y.max(self.clip.y);
        let bottom_y = rect.bottom().min(self.clip.bottom());
        for y in top_y..bottom_y {
            let dy = if y < rect.y + radius {
                (rect.y + radius - y) as f32
            } else if y >= rect.bottom() - radius {
                (y - (rect.bottom() - radius - 1)) as f32
            } else {
                0.0
            };
            let inset = if dy > 0.0 {
                r - (r * r - dy * dy).max(0.0).sqrt()
            } else {
                0.0
            };
            let t = (y - rect.y) as f32 / (rect.h - 1).max(1) as f32;
            let color = top.lerp(bottom, t);
            self.span(y, rect.x as f32 + inset, rect.right() as f32 - inset, color);
        }
    }

    pub fn fill_polygon(&mut self, points: &[(i32, i32)], color: Color) {        if points.len() < 3 {
            return;
        }
        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = points.iter().map(|p| p.1).max().unwrap_or(0);
        for y in min_y..=max_y {
            let mut intersections: Vec<i32> = Vec::new();
            for i in 0..points.len() {
                let (x0, y0) = points[i];
                let (x1, y1) = points[(i + 1) % points.len()];
                if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                    let t = (y - y0) as f32 / (y1 - y0) as f32;
                    intersections.push(x0 + ((x1 - x0) as f32 * t) as i32);
                }
            }
            intersections.sort();
            let mut index = 0;
            while index + 1 < intersections.len() {
                let from = intersections[index];
                let to = intersections[index + 1];
                for x in from..=to {
                    self.put(x, y, color);
                }
                index += 2;
            }
        }
    }

    pub fn blit_alpha(&mut self, x: i32, y: i32, width: i32, height: i32, pixels: &[u32]) {        for row in 0..height {
            for col in 0..width {
                let color = Color(pixels[(row * width + col) as usize]);
                self.put(x + col, y + row, color);
            }
        }
    }
}

/// Build a soft drop shadow mask for a rounded rectangle. The mask is
/// `w + 2*margin` by `h + 2*margin`, 8-bit alpha, black.
pub fn shadow_mask(width: i32, height: i32, radius: i32, margin: i32, blur: i32) -> Vec<u8> {
    let w = width + margin * 2;
    let h = height + margin * 2;
    let mut mask = vec![0u8; (w * h) as usize];
    let r = radius.max(0);
    let hx = margin + width / 2;
    let hy = margin + height / 2;
    let half_w = width / 2;
    let half_h = height / 2;
    let blur = blur.max(1);
    for y in 0..h {
        for x in 0..w {
            let dx = (x - hx).abs() - (half_w - r);
            let dy = (y - hy).abs() - (half_h - r);
            // Approximate the rounded-corner distance without a square root.
            let outside = if dx > 0 && dy > 0 {
                dx.max(dy) + dx.min(dy) * 2 / 5
            } else {
                dx.max(dy)
            };
            let distance = outside.max(0);
            if distance >= blur {
                continue;
            }
            let falloff = ((blur - distance) * 255 / blur) as u32;
            let alpha = (falloff * falloff * 140 / (255 * 255)) as u8;
            mask[(y * w + x) as usize] = alpha;
        }
    }
    mask
}

/// Three-pass box blur used for the wallpaper backdrop.
pub fn box_blur(source: &[u32], width: i32, height: i32, radius: i32) -> Vec<u32> {    let count = (width * height) as usize;
    let mut a: Vec<u32> = vec![0; count];
    let mut b: Vec<u32> = vec![0; count];
    a.copy_from_slice(&source[..count]);
    let radius = radius.max(1);
    for _ in 0..3 {
        // Horizontal.
        for y in 0..height {
            let row = (y * width) as usize;
            let mut sum_r = 0u32;
            let mut sum_g = 0u32;
            let mut sum_b = 0u32;
            for x in -radius..=radius {
                let c = a[row + x.clamp(0, width - 1) as usize];
                sum_r += (c >> 16) & 0xFF;
                sum_g += (c >> 8) & 0xFF;
                sum_b += c & 0xFF;
            }
            let divisor = (radius * 2 + 1) as u32;
            for x in 0..width {
                b[row + x as usize] =
                    ((sum_r / divisor) << 16) | ((sum_g / divisor) << 8) | (sum_b / divisor);
                let out = a[row + (x - radius).clamp(0, width - 1) as usize];
                let inx = a[row + (x + radius + 1).clamp(0, width - 1) as usize];
                sum_r += ((inx >> 16) & 0xFF) - ((out >> 16) & 0xFF);
                sum_g += ((inx >> 8) & 0xFF) - ((out >> 8) & 0xFF);
                sum_b += (inx & 0xFF) - (out & 0xFF);
            }
        }
        // Vertical.
        for x in 0..width {
            let mut sum_r = 0u32;
            let mut sum_g = 0u32;
            let mut sum_b = 0u32;
            for y in -radius..=radius {
                let c = b[(y.clamp(0, height - 1) * width + x) as usize];
                sum_r += (c >> 16) & 0xFF;
                sum_g += (c >> 8) & 0xFF;
                sum_b += c & 0xFF;
            }
            let divisor = (radius * 2 + 1) as u32;
            for y in 0..height {
                a[(y * width + x) as usize] =
                    ((sum_r / divisor) << 16) | ((sum_g / divisor) << 8) | (sum_b / divisor);
                let out = b[((y - radius).clamp(0, height - 1) * width + x) as usize];
                let inx = b[((y + radius + 1).clamp(0, height - 1) * width + x) as usize];
                sum_r += ((inx >> 16) & 0xFF) - ((out >> 16) & 0xFF);
                sum_g += ((inx >> 8) & 0xFF) - ((out >> 8) & 0xFF);
                sum_b += (inx & 0xFF) - (out & 0xFF);
            }
        }
    }
    a
}

/// Integer nearest-neighbour upscale. Cheap enough for the compositor: it is
/// used once for the wallpaper and the pre-blurred glass backdrop.
pub fn upscale_nearest(
    source: &[u32],
    source_width: i32,
    source_height: i32,
    destination: &mut [u32],
    width: i32,
    height: i32,
) {
    if source_width <= 0 || source_height <= 0 {
        return;
    }
    for y in 0..height {
        let sy = (y * source_height / height).clamp(0, source_height - 1);
        let row = (sy * source_width) as usize;
        let out_row = (y * width) as usize;
        for x in 0..width {
            let sx = (x * source_width / width).clamp(0, source_width - 1);
            destination[out_row + x as usize] = source[row + sx as usize];
        }
    }
}

/// Bilinear upscale of a 32-bit image. Used to build the wallpaper and glass
/// backdrop at low cost while keeping smooth gradients.
pub fn upscale_bilinear(
    source: &[u32],
    source_width: i32,
    source_height: i32,
    destination: &mut [u32],
    width: i32,
    height: i32,
) {
    if source_width <= 0 || source_height <= 0 {
        return;
    }
    for y in 0..height {
        let sy = (y as f32 + 0.5) * source_height as f32 / height as f32 - 0.5;
        let y0 = sy.floor().clamp(0.0, (source_height - 1) as f32) as i32;
        let y1 = (y0 + 1).min(source_height - 1);
        let ty = (sy - y0 as f32).clamp(0.0, 1.0);
        for x in 0..width {
            let sx = (x as f32 + 0.5) * source_width as f32 / width as f32 - 0.5;
            let x0 = sx.floor().clamp(0.0, (source_width - 1) as f32) as i32;
            let x1 = (x0 + 1).min(source_width - 1);
            let tx = (sx - x0 as f32).clamp(0.0, 1.0);
            let a = source[(y0 * source_width + x0) as usize];
            let b = source[(y0 * source_width + x1) as usize];
            let c = source[(y1 * source_width + x0) as usize];
            let d = source[(y1 * source_width + x1) as usize];
            let mix = |ch: u32| {
                let top = ((a >> ch) & 0xFF) as f32 * (1.0 - tx) + ((b >> ch) & 0xFF) as f32 * tx;
                let bottom = ((c >> ch) & 0xFF) as f32 * (1.0 - tx) + ((d >> ch) & 0xFF) as f32 * tx;
                (top * (1.0 - ty) + bottom * ty) as u32
            };
            destination[(y * width + x) as usize] = (mix(16) << 16) | (mix(8) << 8) | mix(0);
        }
    }
}

/// Nearest-neighbour downscale for the blur source.
pub fn downsample(
    source: &[u32],
    source_width: i32,
    source_height: i32,
    width: i32,
    height: i32,
) -> Vec<u32> {
    let mut out = vec![0u32; (width * height) as usize];
    for y in 0..height {
        let sy = (y * source_height / height).clamp(0, source_height - 1);
        for x in 0..width {
            let sx = (x * source_width / width).clamp(0, source_width - 1);
            out[(y * width + x) as usize] = source[(sy * source_width + sx) as usize];
        }
    }
    out
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[inline]
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

#[inline]
pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0) * (-2.0 * t + 2.0) / 2.0
    }
}

/// Floating point helpers missing from `core` in `no_std` builds.
pub trait FloatMath {
    fn sqrt(self) -> Self;
    fn ceil(self) -> Self;
    fn floor(self) -> Self;
    fn round(self) -> Self;
    fn cos(self) -> Self;
    fn sin(self) -> Self;
    fn powi(self, exponent: i32) -> Self;
    fn powf(self, exponent: Self) -> Self;
    fn ln(self) -> Self;
}

impl FloatMath for f32 {
    fn sqrt(self) -> f32 {
        unsafe {
            core::arch::x86_64::_mm_cvtss_f32(core::arch::x86_64::_mm_sqrt_ss(
                core::arch::x86_64::_mm_set_ss(self),
            ))
        }
    }
    fn ceil(self) -> f32 {
        libm::ceilf(self)
    }
    fn floor(self) -> f32 {
        libm::floorf(self)
    }
    fn round(self) -> f32 {
        libm::roundf(self)
    }
    fn cos(self) -> f32 {
        libm::cosf(self)
    }
    fn sin(self) -> f32 {
        libm::sinf(self)
    }
    fn powi(self, exponent: i32) -> f32 {
        libm::powf(self, exponent as f32)
    }
    fn powf(self, exponent: f32) -> f32 {
        libm::powf(self, exponent)
    }
    fn ln(self) -> f32 {
        libm::logf(self)
    }
}

impl FloatMath for f64 {
    fn sqrt(self) -> f64 {
        libm::sqrt(self)
    }
    fn ceil(self) -> f64 {
        libm::ceil(self)
    }
    fn floor(self) -> f64 {
        libm::floor(self)
    }
    fn round(self) -> f64 {
        libm::round(self)
    }
    fn cos(self) -> f64 {
        libm::cos(self)
    }
    fn sin(self) -> f64 {
        libm::sin(self)
    }
    fn powi(self, exponent: i32) -> f64 {
        libm::pow(self, exponent as f64)
    }
    fn powf(self, exponent: f64) -> f64 {
        libm::pow(self, exponent)
    }
    fn ln(self) -> f64 {
        libm::log(self)
    }
}
