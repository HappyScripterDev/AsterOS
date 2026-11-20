//! Aster design tokens: palettes, typography, motion, spacing.

use crate::gfx::FloatMath;
use crate::gfx::Color;

pub const TOP_BAR_HEIGHT: i32 = 28;
pub const DOCK_HEIGHT: i32 = 64;
pub const DOCK_GAP: i32 = 8;

pub const ACCENTS: [Accent; 6] = [
    Accent { name: "Blue", dark: 0x5B8CFF, strong: 0x2F6BFF, light: 0x3B6FE8 },
    Accent { name: "Violet", dark: 0xA78BFA, strong: 0x7C5CF0, light: 0x6B4BE0 },
    Accent { name: "Teal", dark: 0x2DD4BF, strong: 0x0FA99A, light: 0x0C8F83 },
    Accent { name: "Rose", dark: 0xFB7185, strong: 0xE8556D, light: 0xD63F5A },
    Accent { name: "Amber", dark: 0xFBBF24, strong: 0xC98A00, light: 0xA87400 },
    Accent { name: "Green", dark: 0x34D399, strong: 0x0F9D6B, light: 0x0B855B },
];

#[derive(Clone, Copy)]
pub struct Accent {
    pub name: &'static str,
    pub dark: u32,
    pub strong: u32,
    pub light: u32,
}

impl Accent {
    pub fn color(&self, dark_theme: bool) -> Color {
        Color::from_hex(if dark_theme { self.dark } else { self.light })
    }

    pub fn strong(&self) -> Color {
        Color::from_hex(self.strong)
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color,
    pub surface: Color,
    pub elevated: Color,
    pub hairline: Color,
    pub text: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub glass: Color,
    pub shadow: Color,
}

pub const DARK: Palette = Palette {
    canvas: Color::from_hex(0x0B0D12),
    surface: Color::from_hex(0x141821),
    elevated: Color::from_hex(0x1B202A),
    hairline: Color::from_hex(0x2A3140),
    text: Color::from_hex(0xF2F4F8),
    secondary: Color::from_hex(0xA8B0BF),
    tertiary: Color::from_hex(0x6F7889),
    glass: Color::rgba(18, 22, 31, 184),
    shadow: Color::rgba(0, 0, 0, 200),
};

pub const LIGHT: Palette = Palette {
    canvas: Color::from_hex(0xEDF0F5),
    surface: Color::from_hex(0xFFFFFF),
    elevated: Color::from_hex(0xF7F9FC),
    hairline: Color::from_hex(0xD8DEE8),
    text: Color::from_hex(0x12151B),
    secondary: Color::from_hex(0x5A6473),
    tertiary: Color::from_hex(0x8A93A3),
    glass: Color::rgba(255, 255, 255, 174),
    shadow: Color::rgba(20, 30, 50, 90),
};

pub const SUCCESS: Color = Color::rgb(0x34, 0xD3, 0x99);
pub const WARNING: Color = Color::rgb(0xFB, 0xBF, 0x24);
pub const DANGER: Color = Color::rgb(0xF8, 0x71, 0x71);
pub const INFO: Color = Color::rgb(0x60, 0xA5, 0xFA);

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub accent_index: usize,
    pub animation_speed: f32,
    pub reduce_motion: bool,
    pub wallpaper: usize,
    pub brightness: f32,
}

impl Theme {
    pub fn palette(&self) -> Palette {
        if self.dark {
            DARK
        } else {
            LIGHT
        }
    }

    pub fn accent(&self) -> Accent {
        ACCENTS[self.accent_index.min(ACCENTS.len() - 1)]
    }

    pub fn accent_color(&self) -> Color {
        self.accent().color(self.dark)
    }

    pub fn accent_strong(&self) -> Color {
        self.accent().strong()
    }

    pub fn surface_glass(&self) -> Color {
        self.palette().glass
    }

    pub fn motion(&self, base_ms: f32) -> f32 {
        if self.reduce_motion {
            0.0
        } else {
            base_ms * self.animation_speed
        }
    }

    pub fn text_on_accent(&self) -> Color {
        let accent = self.accent_color();
        let luminance = 0.2126 * accent.r() as f32 + 0.7152 * accent.g() as f32 + 0.0722 * accent.b() as f32;
        if luminance > 150.0 {
            Color::rgb(20, 22, 28)
        } else {
            Color::rgb(255, 255, 255)
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            dark: true,
            accent_index: 0,
            animation_speed: 1.0,
            reduce_motion: false,
            wallpaper: 0,
            brightness: 1.0,
        }
    }
}

pub fn wallpaper_name(index: usize) -> &'static str {
    match index {
        0 => "Aurora",
        1 => "Tide",
        2 => "Ember",
        3 => "Mint",
        _ => "Aurora",
    }
}

pub const WALLPAPER_COUNT: usize = 4;

/// Generate a procedural wallpaper. No external assets, fully deterministic.
pub fn paint_wallpaper(pixels: &mut [u32], width: i32, height: i32, style: usize, dark: bool) {
    let (top, bottom, glow_a, glow_b) = match style {
        0 => (
            if dark { 0x0A0D18 } else { 0xDDE6FA },
            if dark { 0x141B2E } else { 0xF2E9F7 },
            if dark { 0x2B3D7A } else { 0x9DB8FF },
            if dark { 0x5A2E6E } else { 0xF3C4DE },
        ),
        1 => (
            if dark { 0x071318 } else { 0xDDF1F2 },
            if dark { 0x0C2630 } else { 0xEAF7F0 },
            if dark { 0x12546A } else { 0x8FD8D8 },
            if dark { 0x1F6E58 } else { 0xBDEBD0 },
        ),
        2 => (
            if dark { 0x140D10 } else { 0xF7E8DE },
            if dark { 0x2A1418 } else { 0xF2D8C9 },
            if dark { 0x7A2E2A } else { 0xFFB38F },
            if dark { 0x8A5A1E } else { 0xF7D9A0 },
        ),
        _ => (
            if dark { 0x08120F } else { 0xE8F5EC },
            if dark { 0x10251D } else { 0xEFF9F2 },
            if dark { 0x1E6B52 } else { 0xA8E6C4 },
            if dark { 0x2C5A7A } else { 0xC6E4F7 },
        ),
    };
    let top = Color::from_hex(top);
    let bottom = Color::from_hex(bottom);
    let glow_a = Color::from_hex(glow_a);
    let glow_b = Color::from_hex(glow_b);

    // Base vertical gradient with a soft vignette.
    for y in 0..height {
        let t = y as f32 / (height - 1).max(1) as f32;
        let row = top.lerp(bottom, t);
        for x in 0..width {
            pixels[(y * width + x) as usize] = row.0 & 0x00FF_FFFF;
        }
    }

    // Soft light blooms (radial gradients).
    let blooms = [
        (0.22, 0.30, 0.42, glow_a, 0.55),
        (0.78, 0.62, 0.50, glow_b, 0.48),
        (0.55, 0.12, 0.32, glow_a, 0.25),
        (0.10, 0.82, 0.38, glow_b, 0.22),
    ];
    for (cx, cy, radius, color, strength) in blooms {
        let cx = cx * width as f32;
        let cy = cy * height as f32;
        let radius = radius * width as f32;
        let x0 = (cx - radius).max(0.0) as i32;
        let x1 = (cx + radius).min(width as f32) as i32;
        let y0 = (cy - radius).max(0.0) as i32;
        let y1 = (cy + radius).min(height as f32) as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = (x as f32 - cx) / radius;
                let dy = (y as f32 - cy) / radius;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance < 1.0 {
                    let falloff = (1.0 - distance) * (1.0 - distance) * strength;
                    let dst = Color(pixels[(y * width + x) as usize] | 0xFF00_0000);
                    let mixed = dst.mix(color, falloff);
                    pixels[(y * width + x) as usize] = mixed.0 & 0x00FF_FFFF;
                }
            }
        }
    }

    // Subtle grain so gradients do not band on 8-bit displays.
    let mut seed = 0x9E37_79B9u32 ^ (style as u32).wrapping_mul(0x85EB_CA6B);
    for y in 0..height {
        for x in 0..width {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed & 0x03) as i32 - 1;
            let index = (y * width + x) as usize;
            let c = pixels[index];
            let r = (((c >> 16) & 0xFF) as i32 + noise).clamp(0, 255) as u32;
            let g = (((c >> 8) & 0xFF) as i32 + noise).clamp(0, 255) as u32;
            let b = ((c & 0xFF) as i32 + noise).clamp(0, 255) as u32;
            pixels[index] = (r << 16) | (g << 8) | b;
        }
    }
}
