//! Aster design tokens: light-only macOS-inspired system.

//!

//! The product decision for this build is a single light appearance. There is

//! no dark mode, no theme switch, and no hidden dark palette. The tokens below

//! are the whole visual contract.

use crate::gfx::{fast_sin, Color, FloatMath};

pub const TOP_BAR_HEIGHT: i32 = 26;

pub const DOCK_HEIGHT: i32 = 68;

pub const DOCK_GAP: i32 = 6;

pub const WINDOW_RADIUS: i32 = 12;

pub const TRAFFIC_CLOSE: Color = Color::rgb(0xFF, 0x5F, 0x57);

pub const TRAFFIC_MINIMIZE: Color = Color::rgb(0xFE, 0xBC, 0x2E);

pub const TRAFFIC_MAXIMIZE: Color = Color::rgb(0x28, 0xC8, 0x40);

#[derive(Clone, Copy)]

pub struct Accent {
    pub name: &'static str,

    pub rgb: u32,
}

impl Accent {
    pub fn color(&self) -> Color {
        Color::from_hex(self.rgb)
    }
}

/// macOS-like system accent choices.

pub const ACCENTS: [Accent; 8] = [
    Accent {
        name: "Blue",
        rgb: 0x007AFF,
    },
    Accent {
        name: "Purple",
        rgb: 0xAF52DE,
    },
    Accent {
        name: "Pink",
        rgb: 0xFF2D55,
    },
    Accent {
        name: "Red",
        rgb: 0xFF3B30,
    },
    Accent {
        name: "Orange",
        rgb: 0xFF9500,
    },
    Accent {
        name: "Yellow",
        rgb: 0xFFCC00,
    },
    Accent {
        name: "Green",
        rgb: 0x34C759,
    },
    Accent {
        name: "Graphite",
        rgb: 0x8E8E93,
    },
];

#[derive(Clone, Copy)]

pub struct Palette {
    pub canvas: Color,

    pub surface: Color,

    pub elevated: Color,

    pub sidebar: Color,

    pub hairline: Color,

    pub text: Color,

    pub secondary: Color,

    pub tertiary: Color,

    pub glass: Color,

    pub menu_bar: Color,

    pub shadow: Color,
}

pub const LIGHT: Palette = Palette {
    canvas: Color::from_hex(0xF2F2F7),

    surface: Color::from_hex(0xFFFFFF),

    elevated: Color::from_hex(0xF5F5F7),

    sidebar: Color::from_hex(0xF0F0F3),

    hairline: Color::from_hex(0xD9D9DE),

    text: Color::from_hex(0x1D1D1F),

    secondary: Color::from_hex(0x6E6E73),

    tertiary: Color::from_hex(0x9A9AA1),

    glass: Color::rgba(255, 255, 255, 196),

    menu_bar: Color::rgba(255, 255, 255, 214),

    shadow: Color::rgba(0, 0, 0, 90),
};

pub const SUCCESS: Color = Color::rgb(0x34, 0xC7, 0x59);

pub const WARNING: Color = Color::rgb(0xFF, 0x9F, 0x0A);

pub const DANGER: Color = Color::rgb(0xFF, 0x3B, 0x30);

pub const INFO: Color = Color::rgb(0x00, 0x7A, 0xFF);

#[derive(Clone, Copy, PartialEq)]

pub struct Theme {
    pub accent_index: usize,

    pub animation_speed: f32,

    pub reduce_motion: bool,

    pub wallpaper: usize,

    pub brightness: f32,
}

impl Theme {
    pub fn palette(&self) -> Palette {
        LIGHT
    }

    pub fn accent(&self) -> Accent {
        ACCENTS[self.accent_index.min(ACCENTS.len() - 1)]
    }

    pub fn accent_color(&self) -> Color {
        self.accent().color()
    }

    pub fn accent_strong(&self) -> Color {
        // Slightly darker variant for small text on light surfaces.

        self.accent_color().mix(Color::rgb(0, 0, 0), 0.18)
    }

    pub fn surface_glass(&self) -> Color {
        LIGHT.glass
    }

    pub fn motion(&self, base_ms: f32) -> f32 {
        if self.reduce_motion {
            0.0
        } else {
            base_ms * self.animation_speed
        }
    }

    pub fn text_on_accent(&self) -> Color {
        match self.accent().name {
            "Yellow" | "Green" => Color::rgb(0x1D, 0x1D, 0x1F),

            _ => Color::rgb(255, 255, 255),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
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
        0 => "Sonoma",

        1 => "Sequoia",

        2 => "Ventura",

        3 => "Monterey",

        _ => "Sonoma",
    }
}

pub const WALLPAPER_COUNT: usize = 4;

struct Ribbon {
    offset: f32,

    amplitude: f32,

    frequency_a: f32,

    frequency_b: f32,

    phase: f32,

    width: f32,

    color: (f32, f32, f32),

    strength: f32,
}

struct Style {
    top: (f32, f32, f32),

    bottom: (f32, f32, f32),

    ribbons: [Ribbon; 4],
}

fn style(index: usize) -> Style {
    match index {
        1 => Style {
            top: (0.886, 0.957, 0.906),

            bottom: (0.588, 0.776, 0.667),

            ribbons: [
                Ribbon {
                    offset: 0.30,
                    amplitude: 0.16,
                    frequency_a: 1.0,
                    frequency_b: 2.2,
                    phase: 0.3,
                    width: 0.17,
                    color: (0.149, 0.361, 0.314),
                    strength: 0.45,
                },
                Ribbon {
                    offset: 0.45,
                    amplitude: 0.16,
                    frequency_a: 1.4,
                    frequency_b: 2.7,
                    phase: 1.6,
                    width: 0.15,
                    color: (0.839, 0.945, 0.859),
                    strength: 0.95,
                },
                Ribbon {
                    offset: 0.60,
                    amplitude: 0.18,
                    frequency_a: 0.8,
                    frequency_b: 1.9,
                    phase: 2.4,
                    width: 0.16,
                    color: (0.333, 0.627, 0.471),
                    strength: 0.70,
                },
                Ribbon {
                    offset: 0.78,
                    amplitude: 0.14,
                    frequency_a: 1.5,
                    frequency_b: 3.2,
                    phase: 0.9,
                    width: 0.12,
                    color: (0.965, 0.988, 0.965),
                    strength: 0.95,
                },
            ],
        },

        2 => Style {
            top: (0.992, 0.906, 0.949),

            bottom: (0.745, 0.698, 0.910),

            ribbons: [
                Ribbon {
                    offset: 0.28,
                    amplitude: 0.18,
                    frequency_a: 1.2,
                    frequency_b: 2.6,
                    phase: 0.5,
                    width: 0.16,
                    color: (0.361, 0.404, 0.745),
                    strength: 0.50,
                },
                Ribbon {
                    offset: 0.44,
                    amplitude: 0.16,
                    frequency_a: 1.5,
                    frequency_b: 2.4,
                    phase: 1.7,
                    width: 0.15,
                    color: (1.0, 0.925, 0.965),
                    strength: 0.95,
                },
                Ribbon {
                    offset: 0.58,
                    amplitude: 0.19,
                    frequency_a: 0.9,
                    frequency_b: 2.1,
                    phase: 2.6,
                    width: 0.17,
                    color: (0.769, 0.541, 0.824),
                    strength: 0.65,
                },
                Ribbon {
                    offset: 0.76,
                    amplitude: 0.14,
                    frequency_a: 1.6,
                    frequency_b: 3.3,
                    phase: 1.1,
                    width: 0.12,
                    color: (1.0, 0.973, 0.988),
                    strength: 0.95,
                },
            ],
        },

        3 => Style {
            top: (0.906, 0.957, 0.988),

            bottom: (0.588, 0.737, 0.863),

            ribbons: [
                Ribbon {
                    offset: 0.30,
                    amplitude: 0.17,
                    frequency_a: 1.0,
                    frequency_b: 2.4,
                    phase: 0.2,
                    width: 0.16,
                    color: (0.118, 0.290, 0.463),
                    strength: 0.50,
                },
                Ribbon {
                    offset: 0.46,
                    amplitude: 0.15,
                    frequency_a: 1.4,
                    frequency_b: 2.8,
                    phase: 1.4,
                    width: 0.14,
                    color: (0.886, 0.953, 0.988),
                    strength: 0.95,
                },
                Ribbon {
                    offset: 0.60,
                    amplitude: 0.18,
                    frequency_a: 0.9,
                    frequency_b: 2.0,
                    phase: 2.7,
                    width: 0.16,
                    color: (0.412, 0.639, 0.816),
                    strength: 0.65,
                },
                Ribbon {
                    offset: 0.78,
                    amplitude: 0.13,
                    frequency_a: 1.5,
                    frequency_b: 3.0,
                    phase: 0.8,
                    width: 0.12,
                    color: (0.969, 0.988, 1.0),
                    strength: 0.95,
                },
            ],
        },

        _ => Style {
            top: (0.969, 0.945, 0.910),

            bottom: (0.796, 0.749, 0.694),

            ribbons: [
                Ribbon {
                    offset: 0.30,
                    amplitude: 0.16,
                    frequency_a: 1.1,
                    frequency_b: 2.3,
                    phase: 0.0,
                    width: 0.16,
                    color: (0.431, 0.416, 0.451),
                    strength: 0.55,
                },
                Ribbon {
                    offset: 0.44,
                    amplitude: 0.14,
                    frequency_a: 1.6,
                    frequency_b: 2.9,
                    phase: 1.2,
                    width: 0.13,
                    color: (0.957, 0.922, 0.871),
                    strength: 0.95,
                },
                Ribbon {
                    offset: 0.58,
                    amplitude: 0.18,
                    frequency_a: 0.9,
                    frequency_b: 2.1,
                    phase: 2.3,
                    width: 0.17,
                    color: (0.769, 0.698, 0.616),
                    strength: 0.85,
                },
                Ribbon {
                    offset: 0.74,
                    amplitude: 0.15,
                    frequency_a: 1.4,
                    frequency_b: 3.1,
                    phase: 0.7,
                    width: 0.12,
                    color: (0.980, 0.965, 0.937),
                    strength: 0.95,
                },
            ],
        },
    }
}

#[inline]

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);

    t * t * (3.0 - 2.0 * t)
}

/// Procedural light wallpaper: broad flowing ribbons with soft highlights.

/// Generated at a reduced resolution by the caller and upscaled once.

pub fn paint_wallpaper(pixels: &mut [u32], width: i32, height: i32, style_index: usize) {
    let cfg = style(style_index);

    let w = width.max(1) as f32;

    let h = height.max(1) as f32;

    let tau = core::f32::consts::PI * 2.0;

    // Ribbon centre lines depend only on x. Computing them once per column

    // instead of once per pixel is the difference between a usable and an

    // unusable startup on emulated hardware.

    let mut curves = alloc::vec![[0.0f32; 4]; width.max(1) as usize];

    for (x, column) in curves.iter_mut().enumerate() {
        let nx = x as f32 / (w - 1.0);

        for (index, ribbon) in cfg.ribbons.iter().enumerate() {
            column[index] = ribbon.offset
                + ribbon.amplitude * fast_sin(nx * ribbon.frequency_a * tau + ribbon.phase)
                + ribbon.amplitude
                    * 0.45
                    * fast_sin(nx * ribbon.frequency_b * tau + ribbon.phase * 1.7);
        }
    }

    for y in 0..height {
        let ny = y as f32 / (h - 1.0);

        let base_t = ny;

        let mut row = (
            cfg.top.0 + (cfg.bottom.0 - cfg.top.0) * base_t,
            cfg.top.1 + (cfg.bottom.1 - cfg.top.1) * base_t,
            cfg.top.2 + (cfg.bottom.2 - cfg.top.2) * base_t,
        );

        for x in 0..width {
            let nx = x as f32 / (w - 1.0);

            let (mut r, mut g, mut b) = row;

            // Broad key light from the upper left (squared falloff, no sqrt).

            let dx = nx - 0.18;

            let dy = ny - 0.10;

            let dist2 = dx * dx + dy * dy;

            let light = (1.0 - dist2 * 1.8).max(0.0);

            r += light * 0.055;

            g += light * 0.048;

            b += light * 0.040;

            for (index, ribbon) in cfg.ribbons.iter().enumerate() {
                let curve = curves[x as usize][index];

                let d = (ny - curve) / ribbon.width;

                let d2 = d * d;

                let weight = 1.0 / (1.0 + d2 * d2 * d2);

                if weight > 0.003 {
                    let w = weight * ribbon.strength;

                    r += (ribbon.color.0 - r) * w;

                    g += (ribbon.color.1 - g) * w;

                    b += (ribbon.color.2 - b) * w;
                }

                let edge = ((d + 0.55) / 0.12).abs();

                let edge_weight = 1.0 / (1.0 + edge * edge * edge * edge);

                if edge_weight > 0.01 {
                    let e = edge_weight * 0.20;

                    r += (1.0 - r) * e;

                    g += (1.0 - g) * e;

                    b += (1.0 - b) * e;
                }
            }

            let vx = nx - 0.5;

            let vy = ny - 0.5;

            let vignette = 1.0 - 0.13 * (vx * vx + vy * vy) * 2.0;

            r = (r * vignette).clamp(0.0, 1.0);

            g = (g * vignette).clamp(0.0, 1.0);

            b = (b * vignette).clamp(0.0, 1.0);

            // Tiny deterministic grain to avoid banding.

            let mut seed = (x as u32).wrapping_mul(0x9E37_79B9)
                ^ (y as u32).wrapping_mul(0x85EB_CA6B)
                ^ (style_index as u32).wrapping_mul(0xC2B2_AE35);

            seed ^= seed >> 13;

            seed = seed.wrapping_mul(0x27D4_EB2F);

            seed ^= seed >> 15;

            let grain = ((seed & 3) as i32 - 1) as f32 / 255.0;

            let ri = ((r + grain) * 255.0).clamp(0.0, 255.0) as u32;

            let gi = ((g + grain) * 255.0).clamp(0.0, 255.0) as u32;

            let bi = ((b + grain) * 255.0).clamp(0.0, 255.0) as u32;

            pixels[(y * width + x) as usize] = (ri << 16) | (gi << 8) | bi;

            let _ = smoothstep;
        }

        let _ = &mut row;
    }
}
