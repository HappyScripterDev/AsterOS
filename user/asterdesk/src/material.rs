use crate::gfx::Color;

// Material tokens for the light glass surfaces.
// All colors are logical RGBA; blur and noise are real rendering parameters.

#[derive(Clone, Copy)]
pub struct Material {
    pub tint: Color,
    pub blur_radius: i32,
    pub downsample: i32,
    pub saturation: f32,
    pub noise: u8,
    pub highlight: Color,
    pub border: Color,
}

pub const ULTRA_THIN: Material = Material {
    tint: Color::rgba(255, 255, 255, 138),
    blur_radius: 3,
    downsample: 4,
    saturation: 1.08,
    noise: 2,
    highlight: Color::rgba(255, 255, 255, 170),
    border: Color::rgba(255, 255, 255, 190),
};

pub const THIN: Material = Material {
    tint: Color::rgba(255, 255, 255, 178),
    blur_radius: 3,
    downsample: 3,
    saturation: 1.05,
    noise: 2,
    highlight: Color::rgba(255, 255, 255, 190),
    border: Color::rgba(255, 255, 255, 200),
};

pub const REGULAR: Material = Material {
    tint: Color::rgba(255, 255, 255, 205),
    blur_radius: 3,
    downsample: 3,
    saturation: 1.02,
    noise: 2,
    highlight: Color::rgba(255, 255, 255, 205),
    border: Color::rgba(255, 255, 255, 210),
};

pub const THICK: Material = Material {
    tint: Color::rgba(255, 255, 255, 232),
    blur_radius: 4,
    downsample: 2,
    saturation: 1.0,
    noise: 1,
    highlight: Color::rgba(255, 255, 255, 220),
    border: Color::rgba(255, 255, 255, 220),
};

pub const CHROME: Material = Material {
    tint: Color::rgba(255, 255, 255, 218),
    blur_radius: 2,
    downsample: 4,
    saturation: 1.02,
    noise: 1,
    highlight: Color::rgba(255, 255, 255, 210),
    border: Color::rgba(255, 255, 255, 220),
};

pub const PANEL: Material = Material {
    tint: Color::rgba(255, 255, 255, 226),
    blur_radius: 4,
    downsample: 2,
    saturation: 1.0,
    noise: 1,
    highlight: Color::rgba(255, 255, 255, 220),
    border: Color::rgba(255, 255, 255, 225),
};

pub const SIDEBAR: Material = Material {
    tint: Color::rgba(247, 248, 251, 235),
    blur_radius: 2,
    downsample: 4,
    saturation: 1.0,
    noise: 1,
    highlight: Color::rgba(255, 255, 255, 190),
    border: Color::rgba(255, 255, 255, 190),
};

pub const MENU: Material = Material {
    tint: Color::rgba(252, 252, 254, 244),
    blur_radius: 4,
    downsample: 2,
    saturation: 1.0,
    noise: 0,
    highlight: Color::rgba(255, 255, 255, 230),
    border: Color::rgba(255, 255, 255, 220),
};
