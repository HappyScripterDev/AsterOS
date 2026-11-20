//! Shared UI primitives and procedurally drawn Aster icons.

use alloc::string::{String, ToString};

use crate::gfx::FloatMath;

use crate::gfx::{Color, Rect, Surface};

use crate::icons_data;

use crate::logo_data;

use crate::theme::{Theme, DANGER, SUCCESS, WARNING};

#[derive(Clone, Copy, PartialEq, Eq)]

pub enum Icon {
    Terminal,

    Files,

    Settings,

    Calculator,

    Monitor,

    Text,

    Notes,

    Calendar,

    Photos,

    Search,

    Bell,

    Control,

    Wifi,

    Bluetooth,

    Volume,

    Battery,

    ChevronLeft,

    ChevronRight,

    ChevronDown,

    Close,

    Minimize,

    Maximize,

    Plus,

    Trash,

    Save,

    Folder,

    File,

    Home,

    Up,

    Refresh,

    Sun,

    Moon,

    Power,

    Info,

    Lock,

    Palette,

    Display,

    Sound,

    Network,

    User,

    Shield,

    Storage,

    Accessibility,

    Clock,

    Grid,

    Check,
}

pub fn hit(rect: Rect, x: i32, y: i32) -> bool {
    rect.contains(x, y)
}

pub fn truncate(surface: &Surface, text: &str, face: usize, max_width: i32) -> String {
    if surface.text_width(text, face) <= max_width {
        return text.to_string();
    }

    let mut result = String::new();

    let mut width = 0;
    let ellipsis = surface.text_width("...", face);

    for ch in text.chars() {
        let w = surface.char_width(ch, face);

        if width + w + ellipsis > max_width {
            break;
        }

        result.push(ch);

        width += w;
    }

    result.push_str("...");

    result
}

/// Draw text clipped to `max_width` without allocating. Used on per-frame paths
/// such as the Files list, where `truncate` would grow the bump heap forever.
pub fn draw_text_truncated(
    surface: &mut Surface,
    x: i32,
    y: i32,
    text: &str,
    face: usize,
    color: Color,
    max_width: i32,
) {
    if surface.text_width(text, face) <= max_width {
        surface.draw_text(x, y, text, face, color);
        return;
    }

    let ellipsis = surface.text_width("...", face);
    let mut width = 0;
    let mut end = 0usize;
    for (index, ch) in text.char_indices() {
        let w = surface.char_width(ch, face);
        if width + w + ellipsis > max_width {
            break;
        }
        width += w;
        end = index + ch.len_utf8();
    }
    if end > 0 {
        surface.draw_text(x, y, &text[..end], face, color);
    }
    surface.draw_text(x + width, y, "...", face, color);
}

pub fn app_icon_index(icon: Icon) -> Option<usize> {
    match icon {
        Icon::Terminal => Some(0),

        Icon::Files => Some(1),

        Icon::Settings => Some(2),

        Icon::Calculator => Some(3),

        Icon::Monitor => Some(4),

        Icon::Text => Some(5),

        Icon::Notes => Some(6),

        Icon::Calendar => Some(7),

        Icon::Photos => Some(8),

        _ => None,
    }
}

pub fn draw_app_icon(surface: &mut Surface, icon: Icon, rect: Rect, theme: &Theme) {
    if let Some(index) = app_icon_index(icon) {
        let size = icons_data::ICON_SIZE;

        let stride = (size * size * 4) as usize;

        let offset = index * stride;

        if offset + stride <= icons_data::ICONS.len() {
            surface.draw_texture(
                &icons_data::ICONS[offset..offset + stride],
                size,
                size,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
            );

            return;
        }
    }

    // Fallback for monochrome/system icons.

    let accent = theme.accent_color();

    let radius = (rect.w as f32 * 0.24) as i32;

    surface.fill_rounded(rect, radius, accent);

    let previous = surface.set_clip(rect);

    draw_icon(
        surface,
        icon,
        rect.inset(rect.w / 5),
        Color::rgb(255, 255, 255),
        2,
        theme,
    );

    surface.set_clip(previous);
}

pub fn draw_icon(
    surface: &mut Surface,

    icon: Icon,

    rect: Rect,

    color: Color,

    thickness: i32,

    theme: &Theme,
) {
    let (cx, cy) = rect.center();

    let w = rect.w;

    let h = rect.h;

    let thin = thickness.max(1);

    match icon {
        Icon::Terminal => {
            surface.line(rect.x + w / 4, cy - h / 6, rect.x + w / 2 - 1, cy, color);

            surface.line(rect.x + w / 2 - 1, cy, rect.x + w / 4, cy + h / 6, color);

            surface.fill_rect(
                Rect::new(rect.x + w / 2 + 1, cy + h / 6, w / 4, thin),
                color,
            );
        }

        Icon::Files => {
            let folder = Rect::new(rect.x + w / 8, rect.y + h / 3, w * 3 / 4, h / 2);

            surface.fill_rounded(
                Rect::new(rect.x + w / 8, rect.y + h / 4, w / 3, h / 5),
                3,
                color,
            );

            surface.fill_rounded(folder, 4, color);
        }

        Icon::Settings => {
            surface.stroke_circle(cx, cy, w / 4, thin + 1, color);

            for i in 0..8 {
                let angle = i as f32 * core::f32::consts::PI / 4.0;

                let inner = w / 4 + 2;

                let outer = w / 2 - 1;

                let x0 = cx + (angle.cos() * inner as f32) as i32;

                let y0 = cy + (angle.sin() * inner as f32) as i32;

                let x1 = cx + (angle.cos() * outer as f32) as i32;

                let y1 = cy + (angle.sin() * outer as f32) as i32;

                surface.line(x0, y0, x1, y1, color);
            }
        }

        Icon::Calculator => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 5, rect.y + h / 8, w * 3 / 5, h * 3 / 4),
                3,
                thin,
                color,
            );

            surface.fill_rect(
                Rect::new(rect.x + w / 4, rect.y + h / 4, w / 2 - 2, h / 8),
                color,
            );

            for row in 0..2 {
                for col in 0..3 {
                    surface.fill_rect(
                        Rect::new(
                            rect.x + w / 4 + col * (w / 5),
                            rect.y + h / 2 + row * (h / 5),
                            w / 10,
                            h / 10,
                        ),
                        color,
                    );
                }
            }
        }

        Icon::Monitor => {
            let screen = Rect::new(rect.x + w / 6, rect.y + h / 5, w * 2 / 3, h / 2);

            surface.stroke_rounded(screen, 3, thin, color);

            let mut previous = (rect.x + w / 6, cy + h / 8);

            for step in 1..6 {
                let x = rect.x + w / 6 + step * (w * 2 / 3) / 5;

                let y = cy + h / 8 - ((step * 37) % 9 - 3) * h / 24;

                surface.line(previous.0, previous.1, x, y, color);

                previous = (x, y);
            }

            surface.fill_rect(
                Rect::new(cx - w / 8, rect.y + h * 3 / 4, w / 4, thin),
                color,
            );
        }

        Icon::Text => {
            let page = Rect::new(rect.x + w / 4, rect.y + h / 8, w / 2, h * 3 / 4);

            surface.stroke_rounded(page, 2, thin, color);

            for row in 0..4 {
                surface.fill_rect(
                    Rect::new(
                        page.x + 4,
                        page.y + 5 + row * (h / 8),
                        page.w - 8 - (row % 2) * 5,
                        thin,
                    ),
                    color,
                );
            }
        }

        Icon::Notes => {
            let page = Rect::new(rect.x + w / 5, rect.y + h / 7, w * 3 / 5, h * 5 / 7);

            surface.fill_rounded(page, 3, color);

            for row in 0..3 {
                surface.fill_rect(
                    Rect::new(
                        page.x + 4,
                        page.y + 5 + row * 6,
                        page.w - 8 - (row % 2) * 4,
                        1,
                    ),
                    Color::rgba(20, 22, 28, 160),
                );
            }
        }

        Icon::Calendar => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 6, rect.y + h / 5, w * 2 / 3, h * 3 / 5),
                3,
                thin,
                color,
            );

            surface.fill_rect(
                Rect::new(rect.x + w / 6, rect.y + h / 5, w * 2 / 3, 3),
                color,
            );

            for row in 0..2 {
                for col in 0..3 {
                    surface.fill_rect(
                        Rect::new(rect.x + w / 3 + col * 5, rect.y + h / 2 + row * 5, 2, 2),
                        color,
                    );
                }
            }
        }

        Icon::Photos => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 8, rect.y + h / 5, w * 3 / 4, h * 3 / 5),
                3,
                thin,
                color,
            );

            surface.fill_circle(rect.x + w / 3, rect.y + h / 3, w / 12, color);

            let base = rect.y + h * 3 / 4;

            surface.line(rect.x + w / 4, base, rect.x + w / 2, rect.y + h / 2, color);

            surface.line(
                rect.x + w / 2,
                rect.y + h / 2,
                rect.x + w * 3 / 4,
                base,
                color,
            );
        }

        Icon::Search => {
            surface.stroke_circle(cx - 1, cy - 1, w / 3, thin, color);

            surface.line(
                cx + w / 6,
                cy + w / 6,
                rect.right() - 2,
                rect.bottom() - 2,
                color,
            );
        }

        Icon::Bell => {
            surface.fill_circle(cx, cy + h / 8, w / 12, color);

            surface.stroke_rounded(
                Rect::new(cx - w / 4, cy - h / 3, w / 2, h / 2),
                w / 6,
                thin,
                color,
            );

            surface.fill_rect(Rect::new(cx - w / 4, cy + h / 6, w / 2, thin), color);
        }

        Icon::Control => {
            for (offset, value) in [(-h / 4, 0.3), (0, 0.7), (h / 4, 0.5)] {
                surface.fill_rect(Rect::new(rect.x + 2, cy + offset, w - 4, thin), color);

                let knob = rect.x + 2 + ((w - 4) as f32 * value) as i32;

                surface.fill_circle(knob, cy + offset, thin + 1, color);
            }
        }

        Icon::Wifi => {
            for (index, radius) in [w / 3, w / 2].iter().enumerate() {
                surface.stroke_circle(cx, rect.bottom() - 3, *radius, thin, color);

                let _ = index;
            }

            surface.fill_circle(cx, rect.bottom() - 3, thin, color);
        }

        Icon::Bluetooth => {
            surface.line(cx, rect.y + 3, cx, rect.bottom() - 3, color);

            surface.line(cx, rect.y + 3, cx + w / 4, cy - h / 8, color);

            surface.line(cx + w / 4, cy - h / 8, cx - w / 6, cy, color);

            surface.line(cx - w / 6, cy, cx + w / 4, cy + h / 8, color);

            surface.line(cx + w / 4, cy + h / 8, cx, rect.bottom() - 3, color);
        }

        Icon::Volume => {
            surface.fill_rect(Rect::new(rect.x + 2, cy - h / 8, w / 5, h / 4), color);

            surface.line(
                rect.x + 2 + w / 5,
                cy - h / 8,
                rect.x + w / 2,
                cy - h / 3,
                color,
            );

            surface.line(
                rect.x + w / 2,
                cy - h / 3,
                rect.x + w / 2,
                cy + h / 3,
                color,
            );

            surface.line(
                rect.x + w / 2,
                cy + h / 3,
                rect.x + 2 + w / 5,
                cy + h / 8,
                color,
            );

            surface.stroke_circle(cx + w / 4, cy, w / 5, 1, color);
        }

        Icon::Battery => {
            surface.stroke_rounded(
                Rect::new(rect.x + 2, cy - h / 4, w - 6, h / 2),
                2,
                thin,
                color,
            );

            surface.fill_rect(Rect::new(rect.right() - 3, cy - h / 8, 2, h / 4), color);

            surface.fill_rect(
                Rect::new(rect.x + 5, cy - h / 4 + 3, w / 2, h / 2 - 6),
                color,
            );
        }

        Icon::ChevronLeft => {
            surface.line(cx + w / 8, rect.y + h / 4, cx - w / 6, cy, color);

            surface.line(cx - w / 6, cy, cx + w / 8, rect.bottom() - h / 4, color);
        }

        Icon::ChevronRight => {
            surface.line(cx - w / 8, rect.y + h / 4, cx + w / 6, cy, color);

            surface.line(cx + w / 6, cy, cx - w / 8, rect.bottom() - h / 4, color);
        }

        Icon::ChevronDown => {
            surface.line(rect.x + w / 4, cy - h / 8, cx, cy + h / 6, color);

            surface.line(cx, cy + h / 6, rect.right() - w / 4, cy - h / 8, color);
        }

        Icon::Close => {
            surface.line(
                rect.x + w / 4,
                rect.y + h / 4,
                rect.right() - w / 4,
                rect.bottom() - h / 4,
                color,
            );

            surface.line(
                rect.right() - w / 4,
                rect.y + h / 4,
                rect.x + w / 4,
                rect.bottom() - h / 4,
                color,
            );
        }

        Icon::Minimize => {
            surface.fill_rect(Rect::new(rect.x + w / 4, cy, w / 2, thin), color);
        }

        Icon::Maximize => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 4, rect.y + h / 4, w / 2, h / 2),
                2,
                thin,
                color,
            );
        }

        Icon::Plus => {
            surface.fill_rect(Rect::new(cx - thin / 2, rect.y + h / 4, thin, h / 2), color);

            surface.fill_rect(Rect::new(rect.x + w / 4, cy - thin / 2, w / 2, thin), color);
        }

        Icon::Trash => {
            surface.fill_rect(
                Rect::new(rect.x + w / 4, rect.y + h / 3, w / 2, h / 2),
                color,
            );

            surface.fill_rect(
                Rect::new(rect.x + w / 5, rect.y + h / 4, w * 3 / 5, thin),
                color,
            );

            surface.fill_rect(Rect::new(cx - 2, rect.y + h / 6, 4, thin), color);
        }

        Icon::Save => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 5, rect.y + h / 5, w * 3 / 5, h * 3 / 5),
                2,
                thin,
                color,
            );

            surface.fill_rect(
                Rect::new(rect.x + w / 3, rect.y + h / 4, w / 3, h / 6),
                color,
            );

            surface.stroke_rounded(
                Rect::new(rect.x + w / 3, rect.y + h / 2, w / 3, h / 3),
                1,
                thin,
                color,
            );
        }

        Icon::Folder => {
            surface.fill_rounded(
                Rect::new(rect.x + w / 8, rect.y + h / 4, w / 3, h / 5),
                2,
                color,
            );

            surface.stroke_rounded(
                Rect::new(rect.x + w / 8, rect.y + h / 3, w * 3 / 4, h / 2),
                3,
                thin,
                color,
            );
        }

        Icon::File => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 3, rect.y + h / 6, w / 3, h * 2 / 3),
                2,
                thin,
                color,
            );

            for row in 0..3 {
                surface.fill_rect(
                    Rect::new(
                        rect.x + w / 3 + 4,
                        rect.y + h / 3 + row * 5,
                        w / 3 - 8,
                        thin,
                    ),
                    color,
                );
            }
        }

        Icon::Home => {
            surface.line(rect.x + w / 4, cy, cx, rect.y + h / 4, color);

            surface.line(cx, rect.y + h / 4, rect.right() - w / 4, cy, color);

            surface.stroke_rounded(Rect::new(rect.x + w / 3, cy, w / 3, h / 3), 1, thin, color);
        }

        Icon::Up => {
            surface.line(rect.x + w / 4, cy + h / 8, cx, cy - h / 6, color);

            surface.line(cx, cy - h / 6, rect.right() - w / 4, cy + h / 8, color);

            surface.fill_rect(Rect::new(cx - thin / 2, cy - h / 8, thin, h / 2), color);
        }

        Icon::Refresh => {
            surface.stroke_circle(cx, cy, w / 3, thin, color);

            surface.line(cx + w / 4, cy - h / 4, cx + w / 3, cy - h / 3, color);

            surface.line(cx + w / 4, cy - h / 4, cx + w / 2 - 2, cy - h / 6, color);
        }

        Icon::Sun => {
            surface.fill_circle(cx, cy, w / 5, color);

            for i in 0..8 {
                let angle = i as f32 * core::f32::consts::PI / 4.0;

                let inner = w / 3;

                let outer = w / 2 - 1;

                surface.line(
                    cx + (angle.cos() * inner as f32) as i32,
                    cy + (angle.sin() * inner as f32) as i32,
                    cx + (angle.cos() * outer as f32) as i32,
                    cy + (angle.sin() * outer as f32) as i32,
                    color,
                );
            }
        }

        Icon::Moon => {
            surface.fill_circle(cx, cy, w / 3, color);

            surface.fill_circle(cx + w / 6, cy - h / 8, w / 4, theme.palette().elevated);
        }

        Icon::Power => {
            surface.stroke_circle(cx, cy + 2, w / 3, thin, color);

            surface.fill_rect(
                Rect::new(cx - thin / 2, rect.y + 2, thin, h / 2),
                theme.palette().surface,
            );

            surface.fill_rect(Rect::new(cx - thin / 2, rect.y + 2, thin, h / 3), color);
        }

        Icon::Info => {
            surface.stroke_circle(cx, cy, w / 3, thin, color);

            surface.fill_circle(cx, cy - h / 8, 1, color);

            surface.fill_rect(Rect::new(cx - 1, cy - 1, 2, h / 3), color);
        }

        Icon::Lock => {
            surface.stroke_rounded(
                Rect::new(cx - w / 5, cy - h / 3, w * 2 / 5, h / 4),
                w / 8,
                thin,
                color,
            );

            surface.fill_rounded(
                Rect::new(cx - w / 3, cy - h / 12, w * 2 / 3, h / 2),
                2,
                color,
            );

            surface.fill_circle(cx, cy + h / 8, 1, theme.palette().elevated);
        }

        Icon::Palette => {
            surface.stroke_circle(cx, cy, w / 3, thin, color);

            for (dx, dy) in [(-6, -3), (0, -7), (6, -2)] {
                surface.fill_circle(cx + dx, cy + dy, 2, color);
            }
        }

        Icon::Display => {
            surface.stroke_rounded(
                Rect::new(rect.x + w / 8, rect.y + h / 4, w * 3 / 4, h / 2),
                2,
                thin,
                color,
            );

            surface.fill_rect(
                Rect::new(cx - w / 8, rect.y + h * 3 / 4, w / 4, thin),
                color,
            );
        }

        Icon::Sound => {
            surface.fill_rect(Rect::new(rect.x + w / 4, cy - h / 8, w / 6, h / 4), color);

            surface.line(
                rect.x + w / 4 + w / 6,
                cy - h / 8,
                cx + w / 6,
                cy - h / 3,
                color,
            );

            surface.line(cx + w / 6, cy - h / 3, cx + w / 6, cy + h / 3, color);

            surface.line(
                cx + w / 6,
                cy + h / 3,
                rect.x + w / 4 + w / 6,
                cy + h / 8,
                color,
            );
        }

        Icon::Network => {
            surface.stroke_circle(cx, cy, w / 3, thin, color);

            surface.line(rect.x + w / 4, cy, rect.right() - w / 4, cy, color);

            surface.line(cx, rect.y + h / 4, cx, rect.bottom() - h / 4, color);
        }

        Icon::User => {
            surface.fill_circle(cx, cy - h / 5, w / 6, color);

            surface.fill_rounded(
                Rect::new(cx - w / 3, cy + h / 10, w * 2 / 3, h / 4),
                w / 4,
                color,
            );
        }

        Icon::Shield => {
            surface.line(rect.x + w / 5, cy - h / 3, cx, rect.y + h / 5, color);

            surface.line(cx, rect.y + h / 5, rect.right() - w / 5, cy - h / 3, color);

            surface.line(
                rect.right() - w / 5,
                cy - h / 3,
                rect.right() - w / 5,
                cy + h / 8,
                color,
            );

            surface.line(
                rect.x + w / 5,
                cy - h / 3,
                rect.x + w / 5,
                cy + h / 8,
                color,
            );

            surface.line(rect.x + w / 5, cy + h / 8, cx, rect.bottom() - h / 6, color);

            surface.line(
                rect.right() - w / 5,
                cy + h / 8,
                cx,
                rect.bottom() - h / 6,
                color,
            );
        }

        Icon::Storage => {
            for row in 0..2 {
                surface.stroke_rounded(
                    Rect::new(
                        rect.x + w / 5,
                        rect.y + h / 4 + row * h / 3,
                        w * 3 / 5,
                        h / 5,
                    ),
                    2,
                    thin,
                    color,
                );
            }
        }

        Icon::Accessibility => {
            surface.fill_circle(cx, rect.y + h / 4, w / 10, color);

            surface.fill_rect(Rect::new(cx - w / 6, cy - h / 10, w / 3, thin + 1), color);

            surface.line(cx, cy - h / 10, cx - w / 5, rect.bottom() - h / 5, color);

            surface.line(cx, cy - h / 10, cx + w / 5, rect.bottom() - h / 5, color);

            surface.line(cx, cy, cx, rect.bottom() - h / 4, color);
        }

        Icon::Clock => {
            surface.stroke_circle(cx, cy, w / 3, thin, color);

            surface.line(cx, cy, cx, cy - h / 5, color);

            surface.line(cx, cy, cx + w / 5, cy + h / 10, color);
        }

        Icon::Grid => {
            for row in 0..2 {
                for col in 0..2 {
                    surface.fill_rounded(
                        Rect::new(
                            rect.x + w / 5 + col * (w / 2),
                            rect.y + h / 5 + row * (h / 2),
                            w / 4,
                            h / 4,
                        ),
                        2,
                        color,
                    );
                }
            }
        }

        Icon::Check => {
            surface.line(rect.x + w / 4, cy, cx, rect.bottom() - h / 3, color);

            surface.line(
                cx,
                rect.bottom() - h / 3,
                rect.right() - w / 5,
                rect.y + h / 3,
                color,
            );
        }
    }
}

pub fn draw_button(
    surface: &mut Surface,

    rect: Rect,

    label: &str,

    primary: bool,

    theme: &Theme,

    hovered: bool,

    enabled: bool,
) {
    let palette = theme.palette();

    let (background, text) = if !enabled {
        (palette.elevated, palette.tertiary)
    } else if primary {
        let accent = theme.accent_color();

        (
            accent.mix(Color::rgb(255, 255, 255), if hovered { 0.08 } else { 0.0 }),
            theme.text_on_accent(),
        )
    } else {
        (
            Color::rgb(255, 255, 255).mix(palette.text, if hovered { 0.05 } else { 0.0 }),
            palette.text,
        )
    };

    surface.fill_rounded(rect, 7, background);

    if !primary && enabled {
        surface.stroke_rounded(rect, 7, 1, palette.hairline);
    }

    surface.draw_text_centered(
        rect,
        rect.y + (rect.h - 13) / 2,
        label,
        crate::font_data::BODY_MEDIUM,
        text,
    );
}

pub fn draw_toggle(surface: &mut Surface, rect: Rect, on: bool, theme: &Theme) {
    let palette = theme.palette();

    let track = if on {
        theme.accent_color()
    } else {
        Color::rgb(0xE3, 0xE3, 0xE8)
    };

    surface.fill_rounded(rect, rect.h / 2, track);

    let knob_x = if on {
        rect.right() - rect.h / 2
    } else {
        rect.x + rect.h / 2
    };

    surface.fill_circle(
        knob_x,
        rect.y + rect.h / 2,
        rect.h / 2 - 1,
        Color::rgb(255, 255, 255),
    );

    if !on {
        surface.stroke_circle(
            knob_x,
            rect.y + rect.h / 2,
            rect.h / 2 - 1,
            1,
            palette.hairline,
        );
    }
}

pub fn draw_slider(surface: &mut Surface, rect: Rect, value: f32, theme: &Theme) {
    let palette = theme.palette();

    let cy = rect.y + rect.h / 2;

    let track = Rect::new(rect.x, cy - 2, rect.w, 4);

    surface.fill_rounded(track, 2, Color::rgb(0xDD, 0xDD, 0xE2));

    let filled = (rect.w as f32 * value.clamp(0.0, 1.0)) as i32;

    surface.fill_rounded(
        Rect::new(track.x, track.y, filled.max(4), 4),
        2,
        theme.accent_color(),
    );

    let knob = rect.x + filled;

    surface.fill_circle(knob, cy, 8, Color::rgba(0, 0, 0, 28));

    surface.fill_circle(knob, cy - 1, 7, Color::rgb(255, 255, 255));

    surface.stroke_circle(knob, cy - 1, 7, 1, palette.hairline);
}

pub fn draw_text_field(
    surface: &mut Surface,

    rect: Rect,

    text: &str,

    placeholder: &str,

    focused: bool,

    theme: &Theme,
) {
    let palette = theme.palette();

    if focused {
        surface.fill_rounded(rect.inset(-3), 10, theme.accent_color().with_alpha(40));
    }

    surface.fill_rounded(rect, 8, Color::rgb(255, 255, 255));

    surface.stroke_rounded(
        rect,
        8,
        1,
        if focused {
            theme.accent_color()
        } else {
            palette.hairline
        },
    );

    let color = if text.is_empty() {
        palette.tertiary
    } else {
        palette.text
    };

    let shown = if text.is_empty() { placeholder } else { text };

    surface.draw_text(
        rect.x + 10,
        rect.y + (rect.h - 13) / 2,
        shown,
        crate::font_data::BODY,
        color,
    );
}

pub fn draw_card(surface: &mut Surface, rect: Rect, theme: &Theme) {
    let palette = theme.palette();

    surface.fill_rounded(rect, 12, Color::rgb(255, 255, 255));

    surface.stroke_rounded(rect, 12, 1, palette.hairline);
}

/// Finder-style list icon: a filled blue folder or a white document.
pub fn draw_file_icon(
    surface: &mut Surface,
    rect: Rect,
    is_dir: bool,
    selected: bool,
    theme: &Theme,
) {
    let _ = theme;
    if is_dir {
        let base = if selected {
            Color::rgb(255, 255, 255)
        } else {
            Color::rgb(79, 168, 255)
        };
        let light = if selected {
            Color::rgba(255, 255, 255, 220)
        } else {
            Color::rgb(142, 205, 255)
        };
        surface.fill_rounded(
            Rect::new(rect.x, rect.y + 2, rect.w * 2 / 3, rect.h / 4),
            2,
            base,
        );
        surface.fill_rounded(Rect::new(rect.x, rect.y + 4, rect.w, rect.h - 4), 2, base);
        surface.fill_rounded(
            Rect::new(rect.x + 2, rect.y + 6, rect.w - 4, rect.h - 8),
            2,
            light,
        );
    } else {
        let page = if selected {
            Color::rgba(255, 255, 255, 230)
        } else {
            Color::rgb(255, 255, 255)
        };
        let border = if selected {
            Color::rgba(255, 255, 255, 180)
        } else {
            Color::rgba(0, 0, 0, 60)
        };
        let line = if selected {
            Color::rgba(0, 0, 0, 70)
        } else {
            Color::rgb(160, 160, 168)
        };
        surface.fill_rounded(
            Rect::new(rect.x + 2, rect.y + 1, rect.w - 4, rect.h - 2),
            2,
            page,
        );
        surface.stroke_rounded(
            Rect::new(rect.x + 2, rect.y + 1, rect.w - 4, rect.h - 2),
            2,
            1,
            border,
        );
        for row in 0..3 {
            let width = rect.w - 8 - (row % 2) * 3;
            surface.fill_rect(Rect::new(rect.x + 4, rect.y + 5 + row * 4, width, 1), line);
        }
    }
}

pub fn draw_color_tile(surface: &mut Surface, rect: Rect, color: Color, icon: Icon, theme: &Theme) {
    surface.fill_rounded(rect, (rect.w as f32 * 0.26) as i32, color);

    draw_icon(
        surface,
        icon,
        rect.inset(rect.w / 5),
        Color::rgb(255, 255, 255),
        2,
        theme,
    );
}

/// Brand tile for the menu bar and notifications. The supplied logo is a wide,

/// detailed mark; at 16 px its silhouette becomes a blob, so it lives on a

/// small dark squircle the way an app icon would.

pub fn draw_logo_tile(surface: &mut Surface, rect: Rect) {
    let radius = (rect.w as f32 * 0.28) as i32;

    surface.fill_rounded_gradient(rect, radius, Color::rgb(58, 58, 62), Color::rgb(24, 24, 27));

    surface.stroke_rounded(rect, radius, 1, Color::rgba(255, 255, 255, 26));

    let logo_height = (rect.h as f32 * 0.52) as i32;

    let logo_width =
        (logo_height as f32 * logo_data::MASK_WIDTH as f32 / logo_data::MASK_HEIGHT as f32) as i32;

    surface.draw_mask(
        logo_data::MASK,
        logo_data::MASK_WIDTH,
        logo_data::MASK_HEIGHT,
        rect.x + (rect.w - logo_width) / 2,
        rect.y + (rect.h - logo_height) / 2,
        logo_width,
        logo_height,
        Color::rgb(255, 255, 255),
    );
}

pub fn draw_separator(surface: &mut Surface, rect: Rect, theme: &Theme) {
    surface.fill_rect(
        Rect::new(rect.x, rect.y, rect.w, 1),
        theme.palette().hairline,
    );
}

pub fn status_color(status: &str) -> Color {
    match status {
        "ok" | "passed" | "ready" => SUCCESS,

        "warn" | "limited" => WARNING,

        "error" | "missing" => DANGER,

        _ => Color::rgb(0x60, 0xA5, 0xFA),
    }
}
