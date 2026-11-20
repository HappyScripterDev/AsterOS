//! Window management for Aster Desktop.
//!
//! Windows are plain records with a z-order, a private address-space app
//! instance, and animation timestamps. The manager owns input routing for
//! dragging, resizing, snapping and traffic-light controls.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use aster_abi::InputEvent;

use crate::apps::{Action, App, AppKind};
use crate::font_data::{BODY, BODY_MEDIUM, CAPTION};
use crate::gfx::{self, ease_out, Color, Rect, Surface};
use crate::theme::Theme;
use crate::ui::{self, Icon};
use crate::sys;

pub const TITLE_HEIGHT: i32 = 30;
const MIN_WIDTH: i32 = 360;
const MIN_HEIGHT: i32 = 240;
const SHADOW_MARGIN: i32 = 24;
const SNAP_EDGE: i32 = 8;

pub struct Window {
    pub id: u64,
    pub app: App,
    pub rect: Rect,
    pub normal: Rect,
    pub desktop: usize,
    pub minimized: bool,
    pub maximized: bool,
    pub snapped: u8,
    pub z: u64,
    pub opened_at: u64,
    pub closing_at: Option<u64>,
    pub minimize_at: Option<u64>,
    pub shadow: Vec<u8>,
    pub shadow_size: (i32, i32),
    pub title: String,
}

impl Window {
    pub fn content(&self) -> Rect {
        Rect::new(
            self.rect.x,
            self.rect.y + TITLE_HEIGHT,
            self.rect.w,
            self.rect.h - TITLE_HEIGHT,
        )
    }

    pub fn kind(&self) -> AppKind {
        self.app.kind()
    }
}

#[derive(Clone, Copy)]
enum Interaction {
    Drag { id: u64, dx: i32, dy: i32 },
    Resize { id: u64, edges: u8, start_x: i32, start_y: i32, rect: Rect },
}

pub struct WindowManager {
    pub windows: Vec<Window>,
    next_id: u64,
    z_counter: u64,
    pub focus: Option<u64>,
    interaction: Option<Interaction>,
    pub desktop: usize,
    pub desktop_count: usize,
    last_title_click: (u64, u64),
    pub snap_preview: Option<Rect>,
    pub hover_control: Option<(u64, u8)>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_id: 1,
            z_counter: 10,
            focus: None,
            interaction: None,
            desktop: 0,
            desktop_count: 3,
            last_title_click: (0, 0),
            snap_preview: None,
            hover_control: None,
        }
    }

    pub fn open(&mut self, kind: AppKind, theme: &Theme, work_area: Rect) -> u64 {
        if let Some(window) = self.windows.iter_mut().find(|w| w.kind() == kind && w.closing_at.is_none()) {
            let id = window.id;
            if window.desktop != self.desktop {
                window.desktop = self.desktop;
            }
            window.minimized = false;
            window.minimize_at = None;
            self.focus_id(id);
            return id;
        }
        let index = self.windows.len() as i32;
        let (width, height) = default_size(kind);
        let width = width.min(work_area.w - 40);
        let height = height.min(work_area.h - 20);
        let offset = (index % 5) * 24;
        let x = work_area.x + (work_area.w - width) / 2 + offset - 48;
        let y = work_area.y + 32 + offset;
        let rect = Rect::new(x.max(work_area.x + 8), y, width, height);
        let id = self.next_id;
        self.next_id += 1;
        self.z_counter += 1;
        let window = Window {
            id,
            app: App::new(kind, theme),
            rect,
            normal: rect,
            desktop: self.desktop,
            minimized: false,
            maximized: false,
            snapped: 0,
            z: self.z_counter,
            opened_at: sys::time_ms(),
            closing_at: None,
            minimize_at: None,
            shadow: Vec::new(),
            shadow_size: (0, 0),
            title: String::new(),
        };
        self.windows.push(window);
        self.focus = Some(id);
        id
    }

    pub fn open_path(&mut self, path: &str, theme: &Theme, work_area: Rect) -> u64 {
        let id = self.open(AppKind::Text, theme, work_area);
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.app.open_path(path);
        }
        id
    }

    pub fn focus_id(&mut self, id: u64) {
        self.z_counter += 1;
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.z = self.z_counter;
            self.focus = Some(id);
        }
    }

    pub fn focused(&self) -> Option<&Window> {
        let id = self.focus?;
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Window> {
        let id = self.focus?;
        self.windows.iter_mut().find(|w| w.id == id)
    }

    pub fn visible_on_desktop(&self, desktop: usize) -> impl Iterator<Item = &Window> {
        self.windows
            .iter()
            .filter(move |w| w.desktop == desktop && w.closing_at.is_none())
    }

    pub fn running(&self, kind: AppKind) -> bool {
        self.windows.iter().any(|w| w.kind() == kind && w.closing_at.is_none())
    }

    pub fn close(&mut self, id: u64) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.closing_at = Some(sys::time_ms());
        }
        if self.focus == Some(id) {
            self.focus = self
                .windows
                .iter()
                .filter(|w| w.id != id && w.closing_at.is_none() && w.desktop == self.desktop)
                .max_by_key(|w| w.z)
                .map(|w| w.id);
        }
    }

    pub fn minimize(&mut self, id: u64) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.minimized = true;
            window.minimize_at = Some(sys::time_ms());
        }
        if self.focus == Some(id) {
            self.focus = None;
        }
    }

    pub fn restore(&mut self, id: u64) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.minimized = false;
            window.minimize_at = None;
        }
        self.focus_id(id);
    }

    pub fn toggle_maximize(&mut self, id: u64, work_area: Rect) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            if window.maximized {
                window.maximized = false;
                window.snapped = 0;
                window.rect = window.normal;
            } else {
                window.normal = window.rect;
                window.maximized = true;
                window.snapped = 0;
                window.rect = Rect::new(
                    work_area.x + 8,
                    work_area.y + 8,
                    work_area.w - 16,
                    work_area.h - 16,
                );
            }
        }
    }

    pub fn cycle_focus(&mut self) {
        let mut candidates: Vec<&Window> = self
            .windows
            .iter()
            .filter(|w| w.desktop == self.desktop && !w.minimized && w.closing_at.is_none())
            .collect();
        if candidates.is_empty() {
            self.focus = None;
            return;
        }
        candidates.sort_by_key(|w| w.z);
        let next = if let Some(focus) = self.focus {
            candidates
                .iter()
                .position(|w| w.id == focus)
                .map(|index| candidates[(index + 1) % candidates.len()].id)
                .unwrap_or(candidates[0].id)
        } else {
            candidates.last().unwrap().id
        };
        self.focus_id(next);
    }

    pub fn switch_desktop(&mut self, delta: i32) {
        let count = self.desktop_count as i32;
        self.desktop = ((self.desktop as i32 + delta).rem_euclid(count)) as usize;
        self.focus = self
            .windows
            .iter()
            .filter(|w| w.desktop == self.desktop && !w.minimized && w.closing_at.is_none())
            .max_by_key(|w| w.z)
            .map(|w| w.id);
    }

    pub fn move_to_desktop(&mut self, id: u64, desktop: usize) {
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.desktop = desktop;
        }
    }

    fn sorted(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.windows.len()).collect();
        indices.sort_by_key(|&index| self.windows[index].z);
        indices
    }

    pub fn tick(&mut self, theme: &Theme) {
        self.windows.retain(|window| {
            if let Some(start) = window.closing_at {
                if sys::time_ms().saturating_sub(start) > 200 {
                    return false;
                }
            }
            true
        });
        for window in &mut self.windows {
            window.app.tick(theme);
            let title = window.app.title();
            if title != window.title {
                window.title = title;
            }
        }
    }

    pub fn draw(
        &mut self,
        surface: &mut Surface,
        theme: &Theme,
        now: u64,
        dock_anchor: (i32, i32),
        mouse: (i32, i32),
        top_bar: i32,
    ) {
        let order = self.sorted();
        for index in order {
            let window = &mut self.windows[index];
            if window.desktop != self.desktop {
                continue;
            }
            if window.minimized && window.minimize_at.is_none() {
                continue;
            }
            let focused = self.focus == Some(window.id);
            let opening = ease_out(
                (now.saturating_sub(window.opened_at) as f32 / 190.0).clamp(0.0, 1.0),
            );
            let mut rect = window.rect;
            rect.y += ((1.0 - opening) * 10.0) as i32;
            rect = Rect::new(
                rect.x + ((1.0 - opening) * rect.w as f32 * 0.02) as i32,
                rect.y,
                (rect.w as f32 * (0.98 + 0.02 * opening)) as i32,
                (rect.h as f32 * (0.98 + 0.02 * opening)) as i32,
            );
            if let Some(start) = window.closing_at {
                let t = (now.saturating_sub(start) as f32 / 180.0).clamp(0.0, 1.0);
                rect = Rect::new(
                    rect.x + (rect.w as f32 * t * 0.02) as i32,
                    rect.y + (rect.h as f32 * t * 0.02) as i32,
                    (rect.w as f32 * (1.0 - t * 0.04)) as i32,
                    (rect.h as f32 * (1.0 - t * 0.04)) as i32,
                );
            }
            if let Some(start) = window.minimize_at {
                if window.minimized {
                    let t = (now.saturating_sub(start) as f32 / 220.0).clamp(0.0, 1.0);
                    let eased = t * t * (3.0 - 2.0 * t);
                    let target_x = dock_anchor.0;
                    let target_y = dock_anchor.1;
                    let cx = rect.x as f32 + rect.w as f32 / 2.0;
                    let cy = rect.y as f32 + rect.h as f32 / 2.0;
                    let nx = cx + (target_x as f32 - cx) * eased;
                    let ny = cy + (target_y as f32 - cy) * eased;
                    let scale = 1.0 - eased * 0.82;
                    rect = Rect::new(
                        nx as i32 - (rect.w as f32 * scale) as i32 / 2,
                        ny as i32 - (rect.h as f32 * scale) as i32 / 2,
                        (rect.w as f32 * scale) as i32,
                        (rect.h as f32 * scale) as i32,
                    );
                    if t >= 1.0 {
                        window.minimize_at = None;
                        continue;
                    }
                }
            }
            if rect.y < top_bar + 2 {
                rect.y = top_bar + 2;
            }
            if window.shadow_size != (rect.w, rect.h) {
                window.shadow = gfx::shadow_mask(rect.w, rect.h, 14, SHADOW_MARGIN, 22);
                window.shadow_size = (rect.w, rect.h);
            }
            surface.rounded_shadow(
                &window.shadow,
                rect.x - SHADOW_MARGIN,
                rect.y - SHADOW_MARGIN,
                rect.w + SHADOW_MARGIN * 2,
                rect.h + SHADOW_MARGIN * 2,
            );
            let palette = theme.palette();
            let corner_radius = 14i32.min(rect.w).min(rect.h);
            let mut saved_corners = [0u32; 4 * 14 * 14];
            let mut saved_index = 0usize;
            if corner_radius > 0 {
                for (ox, oy) in [
                    (0, 0),
                    (rect.w - corner_radius, 0),
                    (0, rect.h - corner_radius),
                    (rect.w - corner_radius, rect.h - corner_radius),
                ] {
                    for dy in 0..corner_radius {
                        for dx in 0..corner_radius {
                            saved_corners[saved_index] = surface.pixel(rect.x + ox + dx, rect.y + oy + dy);
                            saved_index += 1;
                        }
                    }
                }
            }
            surface.fill_rounded(rect, 14, palette.surface);
            if focused {
                surface.stroke_rounded(rect, 14, 1, theme.accent_color().with_alpha(90));
            } else {
                surface.stroke_rounded(rect, 14, 1, palette.hairline);
            }

            let titlebar = Rect::new(rect.x, rect.y, rect.w, TITLE_HEIGHT);
            let previous_clip = surface.set_clip(titlebar);
            surface.draw_text_centered(
                titlebar,
                titlebar.y + 8,
                &ui::truncate(surface, &window.title, BODY_MEDIUM, titlebar.w - 150),
                BODY_MEDIUM,
                if focused { palette.text } else { palette.secondary },
            );
            surface.set_clip(previous_clip);
            surface.fill_rect(
                Rect::new(rect.x + 1, rect.y + TITLE_HEIGHT - 1, rect.w - 2, 1),
                palette.hairline,
            );

            let controls = [
                (Icon::Close, Color::rgb(0xFF, 0x5F, 0x57)),
                (Icon::Minimize, Color::rgb(0xFE, 0xBC, 0x2E)),
                (Icon::Maximize, Color::rgb(0x28, 0xC8, 0x40)),
            ];
            for (index, (icon, color)) in controls.iter().enumerate() {
                let cx = rect.x + 18 + index as i32 * 20;
                let cy = rect.y + TITLE_HEIGHT / 2;
                let hovered = self.hover_control == Some((window.id, index as u8));
                let base = if focused { *color } else { palette.hairline };
                surface.fill_circle(cx, cy, 6, if hovered { color.mix(Color::rgb(0, 0, 0), 0.1) } else { base });
                if hovered {
                    ui::draw_icon(
                        surface,
                        *icon,
                        Rect::new(cx - 4, cy - 4, 8, 8),
                        Color::rgba(20, 22, 28, 200),
                        1,
                        theme,
                    );
                }
            }

            let content = Rect::new(rect.x, rect.y + TITLE_HEIGHT, rect.w, rect.h - TITLE_HEIGHT);
            let previous = surface.set_clip(content);
            window.app.draw(surface, content, theme);
            surface.set_clip(previous);

            // Restore the pixels outside the rounded corners so app content
            // cannot square off the window silhouette.
            if corner_radius > 0 {
                let mut index = 0usize;
                for (ox, oy) in [
                    (0, 0),
                    (rect.w - corner_radius, 0),
                    (0, rect.h - corner_radius),
                    (rect.w - corner_radius, rect.h - corner_radius),
                ] {
                    let center_x = rect.x + ox + if ox == 0 { corner_radius } else { 0 };
                    let center_y = rect.y + oy + if oy == 0 { corner_radius } else { 0 };
                    for dy in 0..corner_radius {
                        for dx in 0..corner_radius {
                            let px = rect.x + ox + dx;
                            let py = rect.y + oy + dy;
                            let ddx = px - center_x;
                            let ddy = py - center_y;
                            if ddx * ddx + ddy * ddy > corner_radius * corner_radius {
                                surface.set_pixel(px, py, saved_corners[index]);
                            }
                            index += 1;
                        }
                    }
                }
            }
            let _ = mouse;
        }

        if let Some(preview) = self.snap_preview {
            surface.fill_rounded(preview, 14, theme.accent_color().with_alpha(40));
            surface.stroke_rounded(preview, 14, 2, theme.accent_color());
        }
    }

    pub fn window_at(&self, x: i32, y: i32) -> Option<u64> {
        let order = self.sorted();
        for index in order.into_iter().rev() {
            let window = &self.windows[index];
            if window.desktop != self.desktop || window.minimized || window.closing_at.is_some() {
                continue;
            }
            if window.rect.contains(x, y) {
                return Some(window.id);
            }
        }
        None
    }

    fn edge_at(window: &Window, x: i32, y: i32) -> Option<u8> {
        let rect = window.rect;
        let border = 6;
        let mut edges = 0u8;
        if (x - rect.x).abs() <= border {
            edges |= 1;
        }
        if (rect.right() - x).abs() <= border {
            edges |= 2;
        }
        if (y - rect.y).abs() <= border {
            edges |= 4;
        }
        if (rect.bottom() - y).abs() <= border {
            edges |= 8;
        }
        if edges != 0 && rect.contains(x, y) {
            Some(edges)
        } else {
            None
        }
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, theme: &Theme, work_area: Rect) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some(id) = self.window_at(x, y) else {
            return actions;
        };
        self.focus_id(id);
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return actions;
        };

        // Traffic lights.
        let rect = self.windows[index].rect;
        for control in 0..3u8 {
            let cx = rect.x + 18 + control as i32 * 20;
            let cy = rect.y + TITLE_HEIGHT / 2;
            if (x - cx).abs() <= 9 && (y - cy).abs() <= 9 {
                match control {
                    0 => self.close(id),
                    1 => self.minimize(id),
                    _ => self.toggle_maximize(id, work_area),
                }
                return actions;
            }
        }

        if y < rect.y + TITLE_HEIGHT {
            let now = sys::time_ms();
            if self.last_title_click.0 == id && now.saturating_sub(self.last_title_click.1) < 400 {
                self.toggle_maximize(id, work_area);
                self.last_title_click = (0, 0);
                return actions;
            }
            self.last_title_click = (id, now);
            self.interaction = Some(Interaction::Drag {
                id,
                dx: x - rect.x,
                dy: y - rect.y,
            });
            self.snap_preview = None;
            return actions;
        }

        if let Some(edges) = Self::edge_at(&self.windows[index], x, y) {
            self.interaction = Some(Interaction::Resize {
                id,
                edges,
                start_x: x,
                start_y: y,
                rect,
            });
            return actions;
        }

        let content = self.windows[index].content();
        let window = &mut self.windows[index];
        if let Some(action) = window.app.mouse(x, y, content, theme) {
            actions.push(action);
        }
        actions
    }

    pub fn mouse_move(&mut self, x: i32, y: i32, work_area: Rect, top_bar: i32) {
        // Hover state for traffic lights.
        self.hover_control = None;
        if let Some(id) = self.window_at(x, y) {
            if let Some(window) = self.windows.iter().find(|w| w.id == id) {
                if y < window.rect.y + TITLE_HEIGHT {
                    for control in 0..3u8 {
                        let cx = window.rect.x + 18 + control as i32 * 20;
                        let cy = window.rect.y + TITLE_HEIGHT / 2;
                        if (x - cx).abs() <= 9 && (y - cy).abs() <= 9 {
                            self.hover_control = Some((id, control));
                        }
                    }
                }
            }
        }

        match self.interaction {
            Some(Interaction::Drag { id, dx, dy }) => {
                if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                    if window.maximized {
                        let ratio = (x - window.rect.x) as f32 / window.rect.w as f32;
                        window.maximized = false;
                        window.rect = window.normal;
                        window.rect.x = x - (window.rect.w as f32 * ratio) as i32;
                    }
                    window.rect.x = (x - dx).clamp(-window.rect.w + 120, work_area.right() - 120);
                    window.rect.y = (y - dy).clamp(top_bar + 2, work_area.bottom() - 40);
                }
                if x <= SNAP_EDGE {
                    self.snap_preview = Some(Rect::new(work_area.x, work_area.y, work_area.w / 2, work_area.h));
                } else if x >= work_area.right() - SNAP_EDGE {
                    self.snap_preview = Some(Rect::new(work_area.x + work_area.w / 2, work_area.y, work_area.w / 2, work_area.h));
                } else if y <= top_bar + 4 {
                    self.snap_preview = Some(work_area.inset(8));
                } else {
                    self.snap_preview = None;
                }
            }
            Some(Interaction::Resize { id, edges, start_x, start_y, rect }) => {
                let dx = x - start_x;
                let dy = y - start_y;
                let mut new_rect = rect;
                if edges & 1 != 0 {
                    new_rect.x = (rect.x + dx).min(rect.right() - MIN_WIDTH);
                    new_rect.w = rect.right() - new_rect.x;
                }
                if edges & 2 != 0 {
                    new_rect.w = (rect.w + dx).max(MIN_WIDTH);
                }
                if edges & 4 != 0 {
                    new_rect.y = (rect.y + dy).min(rect.bottom() - MIN_HEIGHT).max(top_bar + 2);
                    new_rect.h = rect.bottom() - new_rect.y;
                }
                if edges & 8 != 0 {
                    new_rect.h = (rect.h + dy).max(MIN_HEIGHT);
                }
                if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                    window.rect = new_rect;
                    window.normal = new_rect;
                    window.maximized = false;
                    window.snapped = 0;
                }
            }
            None => {}
        }
    }

    pub fn mouse_up(&mut self, x: i32, y: i32, work_area: Rect) {
        if let Some(Interaction::Drag { id, .. }) = self.interaction {
            let mut snap = 0u8;
            if x <= SNAP_EDGE {
                snap = 1;
            } else if x >= work_area.right() - SNAP_EDGE {
                snap = 2;
            } else if y <= 34 {
                if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                    window.normal = window.rect;
                    window.maximized = true;
                    window.rect = work_area.inset(8);
                }
            }
            if snap != 0 {
                if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                    window.normal = window.rect;
                    window.snapped = snap;
                    window.maximized = false;
                    window.rect = if snap == 1 {
                        Rect::new(work_area.x, work_area.y, work_area.w / 2, work_area.h)
                    } else {
                        Rect::new(work_area.x + work_area.w / 2, work_area.y, work_area.w / 2, work_area.h)
                    };
                }
            }
        }
        self.interaction = None;
        self.snap_preview = None;
    }

    pub fn key(&mut self, event: &InputEvent, theme: &Theme) -> Vec<Action> {
        let mut actions = Vec::new();
        if let Some(window) = self.focused_mut() {
            if let Some(action) = window.app.key(event, theme) {
                actions.push(action);
            }
        }
        actions
    }

    pub fn route_action(&mut self, action: Action, theme: &Theme, work_area: Rect, target: Option<u64>) -> Option<Action> {
        match action {
            Action::Close => {
                if let Some(id) = target.or(self.focus) {
                    self.close(id);
                }
                None
            }
            Action::Launch(kind) => {
                self.open(kind, theme, work_area);
                None
            }
            Action::OpenPath(path) => {
                self.open_path(&path, theme, work_area);
                None
            }
            other => Some(other),
        }
    }

    pub fn focused_title(&self) -> Option<String> {
        self.focused().map(|window| window.title.clone())
    }
}

fn default_size(kind: AppKind) -> (i32, i32) {
    match kind {
        AppKind::Terminal => (720, 440),
        AppKind::Files => (760, 470),
        AppKind::Settings => (820, 540),
        AppKind::Calculator => (360, 500),
        AppKind::Monitor => (780, 540),
        AppKind::Text => (720, 500),
        AppKind::Notes => (760, 480),
        AppKind::Calendar => (660, 500),
        AppKind::Photos => (740, 520),
    }
}
