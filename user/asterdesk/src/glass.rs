//! Live glass materials. The backdrop is sampled from the already composited
//! backbuffer, downsampled, blurred in place with reusable scratch buffers,
//! then tinted back into the rounded panel. No allocation happens after the
//! scratch buffers have grown once.

use alloc::vec::Vec;

use crate::gfx::{Color, FloatMath, Rect, Surface};

fn horizontal(source: &[u32], destination: &mut [u32], width: i32, height: i32, radius: i32) {
    let radius = radius.max(1);
    for y in 0..height {
        let row = (y * width) as usize;
        for x in 0..width {
            let from = (x - radius).max(0);
            let to = (x + radius).min(width - 1);
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            for sx in from..=to {
                let value = source[row + sx as usize];
                r += (value >> 16) & 0xFF;
                g += (value >> 8) & 0xFF;
                b += value & 0xFF;
            }
            let count = (to - from + 1) as u32;
            destination[row + x as usize] = ((r / count) << 16) | ((g / count) << 8) | (b / count);
        }
    }
}

fn vertical(source: &[u32], destination: &mut [u32], width: i32, height: i32, radius: i32) {
    let radius = radius.max(1);
    for y in 0..height {
        let from = (y - radius).max(0);
        let to = (y + radius).min(height - 1);
        let count = (to - from + 1) as u32;
        for x in 0..width {
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            for sy in from..=to {
                let value = source[(sy * width + x) as usize];
                r += (value >> 16) & 0xFF;
                g += (value >> 8) & 0xFF;
                b += value & 0xFF;
            }
            destination[(y * width + x) as usize] =
                ((r / count) << 16) | ((g / count) << 8) | (b / count);
        }
    }
}

fn blur(
    source: &mut [u32],
    scratch: &mut [u32],
    width: i32,
    height: i32,
    radius: i32,
    passes: i32,
) {
    let mut radius = radius.max(1);
    for _ in 0..passes {
        horizontal(source, scratch, width, height, radius);
        vertical(scratch, source, width, height, radius);
        // Keep the kernel inside the selected quality budget.
        radius = (radius * 3 / 4).max(1);
    }
}

pub fn glass_live(
    surface: &mut Surface,
    rect: Rect,
    radius: i32,
    tint: Color,
    base_radius: i32,
    downsample: i32,
    saturation: f32,
    noise: u8,
    small: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
) {
    let rect = rect.intersect(Rect::new(0, 0, surface.width, surface.height));
    if rect.is_empty() {
        return;
    }
    let ds = downsample.clamp(1, 8);
    let width = (rect.w / ds).max(1);
    let height = (rect.h / ds).max(1);
    let count = (width * height) as usize;
    if small.len() < count {
        small.resize(count, 0);
    }
    if scratch.len() < count {
        scratch.resize(count, 0);
    }
    let small = &mut small[..count];
    let scratch = &mut scratch[..count];

    // Capture the composited backdrop before any of it is overwritten.
    for y in 0..height {
        let sy = rect.y + (y * rect.h / height);
        for x in 0..width {
            let sx = rect.x + (x * rect.w / width);
            small[(y * width + x) as usize] = surface.pixel(sx, sy);
        }
    }
    blur(small, scratch, width, height, base_radius, 2);

    let alpha = tint.alpha() as u32;
    let inverse = 255 - alpha;
    let tint_r = tint.r() as i32;
    let tint_g = tint.g() as i32;
    let tint_b = tint.b() as i32;
    let radius_px = radius.min(rect.w / 2).min(rect.h / 2).max(0);
    let r = radius_px as f32;

    for y in rect.y..rect.bottom() {
        let dy = if y < rect.y + radius_px {
            (rect.y + radius_px - y) as f32
        } else if y >= rect.bottom() - radius_px {
            (y - (rect.bottom() - radius_px - 1)) as f32
        } else {
            0.0
        };
        let inset = if dy > 0.0 {
            r - (r * r - dy * dy).max(0.0).sqrt()
        } else {
            0.0
        };
        let from = (rect.x as f32 + inset).ceil() as i32;
        let to = (rect.right() as f32 - inset).floor() as i32;
        if to <= from {
            continue;
        }
        for x in from..to {
            if !surface.clip.contains(x, y) {
                continue;
            }
            let u = ((x - rect.x) * width / rect.w).clamp(0, width - 1);
            let v = ((y - rect.y) * height / rect.h).clamp(0, height - 1);
            let sample = small[(v * width + u) as usize];
            let sr = ((sample >> 16) & 0xFF) as i32;
            let sg = ((sample >> 8) & 0xFF) as i32;
            let sb = (sample & 0xFF) as i32;
            let grey = (sr + sg + sb) / 3;
            let sat_r = grey + (((sr - grey) as f32) * saturation) as i32;
            let sat_g = grey + (((sg - grey) as f32) * saturation) as i32;
            let sat_b = grey + (((sb - grey) as f32) * saturation) as i32;
            let n = if noise == 0 {
                0
            } else {
                (((x * 7 + y * 13) as u32) & 3) as i32 - 1
            };
            let rr =
                ((sat_r * inverse as i32 + tint_r * alpha as i32) / 255 + n).clamp(0, 255) as u32;
            let gg =
                ((sat_g * inverse as i32 + tint_g * alpha as i32) / 255 + n).clamp(0, 255) as u32;
            let bb =
                ((sat_b * inverse as i32 + tint_b * alpha as i32) / 255 + n).clamp(0, 255) as u32;
            surface.put(x, y, Color::rgb(rr as u8, gg as u8, bb as u8));
        }
    }

    // 1 px inner highlight and a soft outer border: cheap, deterministic
    // specular cues that survive on every backdrop.
    if radius_px > 0 {
        surface.fill_rect(
            Rect::new(rect.x + radius_px, rect.y, rect.w - radius_px * 2, 1),
            Color::rgba(255, 255, 255, 110),
        );
    }
    surface.stroke_rounded(rect, radius_px, 1, Color::rgba(255, 255, 255, 190));
}
