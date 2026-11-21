//! Window management for Aster Desktop.
//!
//! Windows are records with a z-order, a ring-3 application instance and an
//! explicit state machine. Geometry and opacity transitions are time-based;
//! input always routes to the target rectangle so dragging and typing never
//! wait for an animation.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use aster_abi::InputEvent;
use aster_motion::{Easing, Tween};

use crate::apps::{Action, App, AppKind};
use crate::font_data::{BODY, BODY_MEDIUM, CAPTION};
use crate::gfx::{self, Color, Rect, Surface};
use crate::render::QualityMode;
use crate::sys;
use crate::theme::Theme;
use crate::ui::{self, Icon};

pub const TITLE_HEIGHT: i32 = 30;

const MIN_WIDTH: i32 = 360;
const MIN_HEIGHT: i32 = 240;
const SNAP_EDGE: i32 = 8;
const OPEN_MS: u64 = 200;
const CLOSE_MS: u64 = 180;
const MINIMIZE_MS: u64 = 260;
const RESTORE_MS: u64 = 260;
const MAXIMIZE_MS: u64 = 230;
const SNAP_MS: u64 = 210;
const FOCUS_MS: u64 = 170;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowState {
    Opening,
    Normal,
    Minimizing,
    Minimized,
    Restoring,
    Closing,
    Maximized,
    Snapped,
}

impl WindowState {
    pub fn is_visible(self) -> bool {
        !matches!(self, WindowState::Minimized | WindowState::Closing)
    }

    pub fn is_closed(self) -> bool {
        matches!(self, WindowState::Closing)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Open,
    Close,
    Minimize,
    Restore,
    Maximize,
    Unmaximize,
    Snap,
}

#[derive(Clone, Copy)]
pub struct Transition {
    pub from: Rect,
    pub to: Rect,
    pub opacity_from: f32,
    pub opacity_to: f32,
    pub start: u64,
    pub duration: u64,
    pub easing: Easing,
    pub kind: TransitionKind,
}

pub struct Window {
    pub id: u64,
    pub app: App,
    pub rect: Rect,
    pub normal: Rect,
    pub desktop: usize,
    pub minimized: bool,
    pub maximized: bool,
    pub snapped: u8,
    pub fullscreen: bool,
    pub z: u64,
    pub state: WindowState,
    pub transition: Option<Transition>,
    pub focus_changed_at: u64,
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

    pub fn current_rect(&self, now: u64) -> Rect {
        match self.transition {
            Some(transition) => {
                let t = transition
                    .easing
                    .apply(transition_progress(transition, now));
                lerp_rect(transition.from, transition.to, t)
            }
            None => self.rect,
        }
    }

    pub fn current_opacity(&self, now: u64) -> f32 {
        if self.state == WindowState::Minimized {
            return 0.0;
        }
        match self.transition {
            Some(transition) => {
                let t = transition
                    .easing
                    .apply(transition_progress(transition, now));
                let value =
                    transition.opacity_from + (transition.opacity_to - transition.opacity_from) * t;
                value.clamp(0.0, 1.0)
            }
            None => 1.0,
        }
    }
}

fn transition_progress(transition: Transition, now: u64) -> f32 {
    let elapsed = now.saturating_sub(transition.start);
    (elapsed as f32 / transition.duration.max(1) as f32).clamp(0.0, 1.0)
}

fn lerp_rect(from: Rect, to: Rect, t: f32) -> Rect {
    Rect::new(
        (from.x as f32 + (to.x - from.x) as f32 * t) as i32,
        (from.y as f32 + (to.y - from.y) as f32 * t) as i32,
        (from.w as f32 + (to.w - from.w) as f32 * t) as i32,
        (from.h as f32 + (to.h - from.h) as f32 * t) as i32,
    )
}

#[derive(Clone, Copy)]
enum Interaction {
    Drag {
        id: u64,
        dx: i32,
        dy: i32,
    },
    Resize {
        id: u64,
        edges: u8,
        start_x: i32,
        start_y: i32,
        rect: Rect,
    },
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

    pub removed_regions: Vec<Rect>,

    pub force_full: bool,
    dock_anchors: [(AppKind, (i32, i32)); 10],
    screen: Rect,
    animation_speed: f32,
    reduce_motion: bool,
    window_animations: bool,
}

impl WindowManager {
    pub fn new() -> Self {
        let mut dock_anchors = [(AppKind::Terminal, (0, 0)); 10];
        for (index, kind) in AppKind::ALL.iter().enumerate() {
            dock_anchors[index] = (*kind, (0, 0));
        }
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

            removed_regions: Vec::new(),

            force_full: false,

            dock_anchors,
            screen: Rect::new(0, 0, 1280, 800),
            animation_speed: 1.0,
            reduce_motion: false,
            window_animations: true,
        }
    }

    /// Dock icon centres for minimize/restore. The desktop updates this every
    /// frame, so a dock magnification or position change is reflected in the
    /// animation target instead of using a stale centre point.
    pub fn set_dock_anchors(&mut self, anchors: [(AppKind, (i32, i32)); 10]) {
        self.dock_anchors = anchors;
    }

    fn dock_anchor(&self, kind: AppKind) -> (i32, i32) {
        self.dock_anchors
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, point)| *point)
            .unwrap_or((0, 0))
    }

    fn duration(&self, base: u64) -> u64 {
        if self.reduce_motion || !self.window_animations {
            0
        } else {
            (base as f32 * self.animation_speed + 0.5) as u64
        }
    }

    fn begin(
        &mut self,
        index: usize,
        kind: TransitionKind,
        from: Rect,
        to: Rect,
        opacity_from: f32,
        opacity_to: f32,
        base_duration: u64,
        easing: Easing,
    ) {
        let now = sys::time_ms();
        let duration = self.duration(base_duration);
        let window = &mut self.windows[index];
        window.transition = Some(Transition {
            from,
            to,
            opacity_from,
            opacity_to,
            start: now,
            duration,
            easing,
            kind,
        });
        if duration == 0 {
            window.transition = None;
            window.rect = to;
            match kind {
                TransitionKind::Close => window.state = WindowState::Closing,
                TransitionKind::Minimize => {
                    window.state = WindowState::Minimized;
                    window.minimized = true;
                }
                TransitionKind::Restore => {
                    window.state = WindowState::Normal;
                    window.minimized = false;
                }
                TransitionKind::Maximize => window.state = WindowState::Maximized,
                TransitionKind::Unmaximize => window.state = WindowState::Normal,
                TransitionKind::Snap => window.state = WindowState::Snapped,
                TransitionKind::Open => window.state = WindowState::Normal,
            }
        }
    }

    pub fn open(&mut self, kind: AppKind, theme: &Theme, work_area: Rect) -> u64 {
        if let Some(window) = self
            .windows
            .iter_mut()
            .find(|w| w.kind() == kind && !w.state.is_closed())
        {
            let id = window.id;
            window.desktop = self.desktop;
            if window.state == WindowState::Minimized || window.state == WindowState::Minimizing {
                self.restore(id);
            } else {
                self.focus_id(id);
            }
            return id;
        }

        let index = self.windows.len() as i32;
        let (width, height) = default_size(kind);
        let width = width.min(work_area.w - 40);
        let height = height.min(work_area.h - 24);
        let offset = (index % 5) * 24;
        let x = work_area.x + (work_area.w - width) / 2 + offset - 48;
        let y = work_area.y + 28 + offset;
        let rect = Rect::new(x.max(work_area.x + 8), y, width, height);
        let id = self.next_id;
        self.next_id += 1;
        self.z_counter += 1;

        let now = sys::time_ms();
        let window = Window {
            id,
            app: App::new(kind, theme),
            rect,
            normal: rect,
            desktop: self.desktop,
            minimized: false,
            maximized: false,
            snapped: 0,
            fullscreen: false,
            z: self.z_counter,
            state: WindowState::Opening,
            transition: None,
            focus_changed_at: now,
            shadow: Vec::new(),
            shadow_size: (0, 0),
            title: String::new(),
        };
        self.windows.push(window);
        self.focus = Some(id);
        self.animation_speed = theme.animation_speed;
        self.reduce_motion = theme.reduce_motion;
        self.window_animations = theme.window_animations;
        if self.reduce_motion || !theme.window_animations {
            self.windows.last_mut().unwrap().state = WindowState::Normal;
        } else {
            let from = Rect::new(
                rect.x + (rect.w as f32 * 0.02) as i32,
                rect.y + 10,
                (rect.w as f32 * 0.96) as i32,
                (rect.h as f32 * 0.96) as i32,
            );
            self.begin(
                self.windows.len() - 1,
                TransitionKind::Open,
                from,
                rect,
                0.0,
                1.0,
                OPEN_MS,
                Easing::CubicOut,
            );
        }
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
        if self.focus != Some(id) {
            if let Some(previous) = self
                .windows
                .iter_mut()
                .find(|window| Some(window.id) == self.focus && window.fullscreen)
            {
                previous.fullscreen = false;
                previous.rect = previous.normal;
                previous.state = WindowState::Normal;
                previous.transition = None;
            }
        }
        self.z_counter += 1;
        if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
            window.z = self.z_counter;
            window.focus_changed_at = sys::time_ms();
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
            .filter(move |w| w.desktop == desktop && !w.state.is_closed())
    }

    pub fn running(&self, kind: AppKind) -> bool {
        self.windows
            .iter()
            .any(|w| w.kind() == kind && !w.state.is_closed())
    }

    pub fn close(&mut self, id: u64) {
        let now = sys::time_ms();
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        if self.windows[index].state == WindowState::Closing {
            return;
        }
        let from = self.windows[index].current_rect(now);
        let opacity = self.windows[index].current_opacity(now);
        self.windows[index].state = WindowState::Closing;
        let to = Rect::new(
            from.x + (from.w as f32 * 0.02) as i32,
            from.y + (from.h as f32 * 0.02) as i32,
            (from.w as f32 * 0.96) as i32,
            (from.h as f32 * 0.96) as i32,
        );
        self.begin(
            index,
            TransitionKind::Close,
            from,
            to,
            opacity,
            0.0,
            CLOSE_MS,
            Easing::CubicIn,
        );

        if self.focus == Some(id) {
            self.focus = self
                .windows
                .iter()
                .filter(|w| {
                    w.id != id
                        && !w.state.is_closed()
                        && w.desktop == self.desktop
                        && w.state != WindowState::Minimized
                })
                .max_by_key(|w| w.z)
                .map(|w| w.id);
        }
    }

    pub fn minimize(&mut self, id: u64) {
        let now = sys::time_ms();
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        if matches!(
            self.windows[index].state,
            WindowState::Minimizing | WindowState::Minimized
        ) {
            return;
        }
        let from = self.windows[index].current_rect(now);
        if self.windows[index].fullscreen {
            self.windows[index].fullscreen = false;
            self.windows[index].rect = self.windows[index].normal;
        }
        let anchor = self.dock_anchor(self.windows[index].kind());
        let target = Rect::new(anchor.0 - 10, anchor.1 - 10, 20, 20);
        self.windows[index].minimized = true;
        self.windows[index].state = WindowState::Minimizing;
        self.begin(
            index,
            TransitionKind::Minimize,
            from,
            target,
            1.0,
            0.0,
            MINIMIZE_MS,
            Easing::CubicInOut,
        );

        if self.focus == Some(id) {
            self.focus = self
                .windows
                .iter()
                .filter(|w| {
                    w.id != id
                        && !w.state.is_closed()
                        && w.state != WindowState::Minimized
                        && w.state != WindowState::Minimizing
                        && w.desktop == self.desktop
                })
                .max_by_key(|w| w.z)
                .map(|w| w.id);
        }
    }

    pub fn restore(&mut self, id: u64) {
        let now = sys::time_ms();
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        let window = &self.windows[index];
        if !matches!(
            window.state,
            WindowState::Minimized | WindowState::Minimizing | WindowState::Restoring
        ) {
            self.focus_id(id);
            return;
        }
        let visible = window.current_rect(now);
        let anchor = self.dock_anchor(window.kind());
        let from = if window.state == WindowState::Minimizing {
            visible
        } else {
            Rect::new(anchor.0 - 10, anchor.1 - 10, 20, 20)
        };
        let to = window.normal;
        self.windows[index].minimized = false;
        self.windows[index].state = WindowState::Restoring;
        self.begin(
            index,
            TransitionKind::Restore,
            from,
            to,
            0.0,
            1.0,
            RESTORE_MS,
            Easing::CubicOut,
        );
        self.focus_id(id);
    }

    pub fn set_screen(&mut self, screen: Rect) {
        self.screen = screen;
    }

    pub fn fullscreen_active(&self) -> bool {
        self.focus
            .and_then(|id| self.windows.iter().find(|window| window.id == id))
            .map(|window| window.fullscreen && window.desktop == self.desktop)
            .unwrap_or(false)
    }

    pub fn toggle_fullscreen(&mut self, id: u64) {
        let now = sys::time_ms();
        let screen = self.screen;
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        let current = self.windows[index].current_rect(now);
        if self.windows[index].fullscreen {
            let target = self.windows[index].normal;
            self.windows[index].fullscreen = false;
            self.windows[index].rect = target;
            self.windows[index].state = WindowState::Normal;
            self.begin(
                index,
                TransitionKind::Unmaximize,
                current,
                target,
                1.0,
                1.0,
                MAXIMIZE_MS,
                Easing::CubicInOut,
            );
        } else {
            self.windows[index].normal = self.windows[index].rect;
            self.windows[index].fullscreen = true;
            self.windows[index].maximized = false;
            self.windows[index].snapped = 0;
            self.windows[index].rect = screen;
            self.windows[index].state = WindowState::Maximized;
            self.begin(
                index,
                TransitionKind::Maximize,
                current,
                screen,
                1.0,
                1.0,
                MAXIMIZE_MS,
                Easing::CubicInOut,
            );
        }
        self.focus_id(id);
    }

    pub fn toggle_maximize(&mut self, id: u64, work_area: Rect) {
        if let Some(window) = self.windows.iter().find(|window| window.id == id) {
            if window.fullscreen {
                self.toggle_fullscreen(id);
                return;
            }
        }
        let now = sys::time_ms();
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        let current = self.windows[index].current_rect(now);
        let maximized = self.windows[index].maximized;
        if maximized {
            let target = self.windows[index].normal;
            self.windows[index].maximized = false;
            self.windows[index].snapped = 0;
            self.windows[index].rect = target;
            self.windows[index].state = WindowState::Normal;
            self.begin(
                index,
                TransitionKind::Unmaximize,
                current,
                target,
                1.0,
                1.0,
                MAXIMIZE_MS,
                Easing::CubicInOut,
            );
        } else {
            self.windows[index].normal = self.windows[index].rect;
            let target = Rect::new(
                work_area.x + 8,
                work_area.y + 8,
                work_area.w - 16,
                work_area.h - 16,
            );
            self.windows[index].maximized = true;
            self.windows[index].snapped = 0;
            self.windows[index].rect = target;
            self.windows[index].state = WindowState::Maximized;
            self.begin(
                index,
                TransitionKind::Maximize,
                current,
                target,
                1.0,
                1.0,
                MAXIMIZE_MS,
                Easing::CubicInOut,
            );
        }
    }

    fn snap_to(&mut self, id: u64, target: Rect, code: u8, preserve_normal: bool) {
        let now = sys::time_ms();
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        let current = self.windows[index].current_rect(now);
        if preserve_normal {
            self.windows[index].normal = self.windows[index].rect;
        }
        self.windows[index].rect = target;
        self.windows[index].snapped = code;
        self.windows[index].maximized = false;
        self.windows[index].state = WindowState::Snapped;
        self.begin(
            index,
            TransitionKind::Snap,
            current,
            target,
            1.0,
            1.0,
            SNAP_MS,
            Easing::CubicOut,
        );
    }

    pub fn cycle_focus(&mut self) {
        let mut candidates: Vec<&Window> = self
            .windows
            .iter()
            .filter(|w| {
                w.desktop == self.desktop
                    && w.state != WindowState::Minimized
                    && !w.state.is_closed()
            })
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
            .filter(|w| {
                w.desktop == self.desktop
                    && w.state != WindowState::Minimized
                    && !w.state.is_closed()
            })
            .max_by_key(|w| w.z)
            .map(|w| w.id);
    }

    pub fn set_desktop(&mut self, desktop: usize) {
        self.desktop = desktop.min(self.desktop_count.saturating_sub(1));
        self.focus = self
            .windows
            .iter()
            .filter(|w| {
                w.desktop == self.desktop
                    && w.state != WindowState::Minimized
                    && !w.state.is_closed()
            })
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
        self.animation_speed = theme.animation_speed;
        self.reduce_motion = theme.reduce_motion;
        self.window_animations = theme.window_animations;
        let now = sys::time_ms();
        self.removed_regions.clear();
        for index in 0..self.windows.len() {
            self.windows[index].app.tick(theme);
            let title = self.windows[index].app.title();
            if title != self.windows[index].title {
                self.windows[index].title = title;
            }
            let finished = self.windows[index]
                .transition
                .map(|transition| now.saturating_sub(transition.start) >= transition.duration)
                .unwrap_or(false);
            if finished {
                let transition = self.windows[index].transition.unwrap();
                let kind = transition.kind;
                if kind == TransitionKind::Minimize {
                    self.removed_regions.push(transition.to.inset(-34));
                    self.force_full = true;
                }
                if kind == TransitionKind::Close {
                    self.force_full = true;
                }
                self.windows[index].transition = None;
                match kind {
                    TransitionKind::Open | TransitionKind::Unmaximize => {
                        self.windows[index].state = WindowState::Normal
                    }
                    TransitionKind::Close => self.windows[index].state = WindowState::Closing,
                    TransitionKind::Minimize => {
                        self.windows[index].state = WindowState::Minimized;
                        self.windows[index].minimized = true;
                    }
                    TransitionKind::Restore => {
                        self.windows[index].state = WindowState::Normal;
                        self.windows[index].minimized = false;
                    }
                    TransitionKind::Maximize => self.windows[index].state = WindowState::Maximized,
                    TransitionKind::Snap => self.windows[index].state = WindowState::Snapped,
                }
            }
        }
        self.windows.retain(|window| {
            !(window.state == WindowState::Closing && window.transition.is_none())
        });
        if let Some(focus) = self.focus {
            if !self.windows.iter().any(|w| w.id == focus) {
                self.focus = self
                    .windows
                    .iter()
                    .filter(|w| {
                        w.desktop == self.desktop
                            && w.state != WindowState::Minimized
                            && !w.state.is_closed()
                    })
                    .max_by_key(|w| w.z)
                    .map(|w| w.id);
            }
        }
    }

    pub fn is_animating(&self, now: u64) -> bool {
        self.interaction.is_some()
            || self.snap_preview.is_some()
            || self.windows.iter().any(|window| {
                window
                    .transition
                    .map(|transition| now.saturating_sub(transition.start) < transition.duration)
                    .unwrap_or(false)
                    || now.saturating_sub(window.focus_changed_at) < FOCUS_MS
            })
    }

    /// Rectangles that changed since the last frame because of a window
    /// transition, drag or focus shadow.
    pub fn animation_regions(&self, now: u64) -> Vec<Rect> {
        let mut regions = Vec::new();
        for window in &self.windows {
            if window.desktop != self.desktop {
                continue;
            }
            if let Some(transition) = window.transition {
                if now.saturating_sub(transition.start) < transition.duration {
                    let union = window
                        .current_rect(now)
                        .union(transition.to)
                        .union(transition.from)
                        .inset(-30);
                    regions.push(union);
                }
            }
            if now.saturating_sub(window.focus_changed_at) < FOCUS_MS {
                regions.push(window.current_rect(now).inset(-24));
            }
        }
        regions.extend(self.removed_regions.iter().copied());
        if let Some(preview) = self.snap_preview {
            regions.push(preview.inset(-4));
        }
        regions
    }

    pub fn visible_bounds(&self) -> Option<Rect> {
        self.windows
            .iter()
            .filter(|window| {
                window.desktop == self.desktop
                    && window.state != WindowState::Minimized
                    && !window.state.is_closed()
            })
            .map(|window| window.current_rect(sys::time_ms()).inset(-30))
            .reduce(|a, b| a.union(b))
    }

    pub fn draw(
        &mut self,
        surface: &mut Surface,
        theme: &Theme,
        now: u64,
        clip: Rect,
        top_bar: i32,
    ) {
        self.draw_for_desktop(surface, theme, now, clip, top_bar, self.desktop, 0);
    }

    /// Draw one workspace at a horizontal offset. Used by the slide transition.
    pub fn draw_for_desktop(
        &mut self,
        surface: &mut Surface,
        theme: &Theme,
        now: u64,
        clip: Rect,
        top_bar: i32,
        desktop: usize,
        offset_x: i32,
    ) {
        let order = self.sorted();
        let quality = QualityMode::from_code(theme.quality);

        for index in order {
            let window = &mut self.windows[index];
            if window.desktop != desktop {
                continue;
            }
            if window.state == WindowState::Minimized {
                continue;
            }
            if window
                .transition
                .map(|transition| transition.kind == TransitionKind::Close)
                .unwrap_or(false)
                && window.current_opacity(now) <= 0.01
            {
                continue;
            }

            let mut rect = window.current_rect(now);
            rect.x += offset_x;
            if rect.intersect(clip.inset(-30)).is_empty() && window.transition.is_none() {
                continue;
            }
            let rect = Rect::new(
                rect.x,
                rect.y.max(top_bar + 2),
                rect.w.max(1),
                rect.h.max(1),
            );
            let opacity = window.current_opacity(now);
            if opacity <= 0.01 {
                continue;
            }

            if window.fullscreen {
                let opacity = window.current_opacity(now);
                let previous_opacity = surface.set_opacity((opacity * 255.0) as u8);
                let content = rect;
                let previous_clip = surface.set_clip(content.intersect(clip));
                window.app.draw(surface, content, theme);
                surface.set_clip(previous_clip);
                surface.set_opacity(previous_opacity);
                continue;
            }

            let focused = self.focus == Some(window.id);
            let focus_t = (now.saturating_sub(window.focus_changed_at) as f32 / FOCUS_MS as f32)
                .clamp(0.0, 1.0);
            let focus_ease = Easing::CubicOut.apply(focus_t);
            let shadow_alpha = if focused {
                150.0 + 70.0 * focus_ease
            } else {
                90.0
            };

            if theme.shadows {
                if window.shadow_size != (rect.w, rect.h) {
                    window.shadow = gfx::shadow_mask(
                        rect.w,
                        rect.h,
                        12,
                        quality.shadow_margin() + 4,
                        quality.shadow_blur(),
                    );
                    window.shadow_size = (rect.w, rect.h);
                }
                let previous =
                    surface.set_opacity(((opacity * shadow_alpha).clamp(0.0, 255.0)) as u8);
                surface.rounded_shadow(
                    &window.shadow,
                    rect.x - quality.shadow_margin() - 4,
                    rect.y - quality.shadow_margin() - 4,
                    rect.w + (quality.shadow_margin() + 4) * 2,
                    rect.h + (quality.shadow_margin() + 4) * 2,
                );
                surface.set_opacity(previous);
            }

            let palette = theme.palette();
            let corner_radius = 12i32.min(rect.w).min(rect.h);
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
                            saved_corners[saved_index] =
                                surface.pixel(rect.x + ox + dx, rect.y + oy + dy);
                            saved_index += 1;
                        }
                    }
                }
            }

            let window_opacity = (opacity * 255.0) as u8;
            let previous_opacity = surface.set_opacity(window_opacity);

            surface.fill_rounded(rect, 12, palette.surface);
            surface.fill_rounded(
                Rect::new(rect.x + 1, rect.y + 1, rect.w - 2, TITLE_HEIGHT),
                11,
                if focused {
                    Color::rgb(250, 250, 252)
                } else {
                    Color::rgb(247, 247, 249)
                },
            );
            surface.stroke_rounded(
                rect,
                12,
                1,
                if focused {
                    Color::rgba(0, 0, 0, 34)
                } else {
                    Color::rgba(0, 0, 0, 20)
                },
            );

            let titlebar = Rect::new(rect.x, rect.y, rect.w, TITLE_HEIGHT);
            let previous_clip =
                surface.set_clip(titlebar.intersect(clip).union(Rect::new(0, 0, 0, 0)));
            surface.draw_text_centered(
                titlebar,
                titlebar.y + 7,
                &ui::truncate(surface, &window.title, BODY_MEDIUM, titlebar.w - 150),
                BODY_MEDIUM,
                if focused {
                    Color::rgb(60, 60, 67)
                } else {
                    palette.tertiary
                },
            );
            surface.set_clip(previous_clip);

            if clip.intersect(titlebar).is_empty() {
                // The title bar is outside this dirty region but the content
                // may still need drawing; continue with the content clip.
            }

            surface.fill_rect(
                Rect::new(rect.x + 1, rect.y + TITLE_HEIGHT - 1, rect.w - 2, 1),
                palette.hairline,
            );

            let controls = [
                (Icon::Close, crate::theme::TRAFFIC_CLOSE),
                (Icon::Minimize, crate::theme::TRAFFIC_MINIMIZE),
                (Icon::Maximize, crate::theme::TRAFFIC_MAXIMIZE),
            ];
            for (index, (icon, color)) in controls.iter().enumerate() {
                let cx = rect.x + 18 + index as i32 * 20;
                let cy = rect.y + TITLE_HEIGHT / 2;
                let hovered = self.hover_control == Some((window.id, index as u8));
                let base = if focused {
                    *color
                } else {
                    Color::rgb(0xD8, 0xD8, 0xDD)
                };
                surface.fill_circle(
                    cx,
                    cy,
                    6,
                    if hovered {
                        color.mix(Color::rgb(0, 0, 0), 0.08)
                    } else {
                        base
                    },
                );
                if hovered {
                    ui::draw_icon(
                        surface,
                        *icon,
                        Rect::new(cx - 4, cy - 4, 8, 8),
                        Color::rgba(45, 45, 50, 210),
                        1,
                        theme,
                    );
                }
            }

            let content = Rect::new(rect.x, rect.y + TITLE_HEIGHT, rect.w, rect.h - TITLE_HEIGHT);
            let previous = surface.set_clip(content.intersect(clip));
            window.app.draw(surface, content, theme);
            surface.set_clip(previous);

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

            surface.set_opacity(previous_opacity);
        }

        if let Some(preview) = self.snap_preview {
            surface.fill_rounded(preview, 12, theme.accent_color().with_alpha(40));
            surface.stroke_rounded(preview, 12, 2, theme.accent_color());
        }
    }

    pub fn window_at(&self, x: i32, y: i32) -> Option<u64> {
        let order = self.sorted();
        for index in order.into_iter().rev() {
            let window = &self.windows[index];
            if window.desktop != self.desktop
                || window.state == WindowState::Minimized
                || window.state == WindowState::Minimizing
                || window.state.is_closed()
            {
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

    pub fn interaction_window(&self) -> Option<u64> {
        match self.interaction {
            Some(Interaction::Drag { id, .. }) => Some(id),
            Some(Interaction::Resize { id, .. }) => Some(id),
            None => None,
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
        if let Some(transition) = self.windows[index].transition {
            if transition.kind == TransitionKind::Close {
                return actions;
            }
        }
        // A new interaction cancels a geometry transition; the target rect is
        // already the authoritative rectangle for input.
        if self.windows[index].state == WindowState::Opening {
            self.windows[index].transition = None;
            self.windows[index].state = WindowState::Normal;
        }

        // Fullscreen windows expose no chrome: the whole surface goes to the
        // app, so hidden traffic lights cannot be clicked and the top edge is
        // not a drag handle.
        if self.windows[index].fullscreen {
            let rect = self.windows[index].rect;
            let window = &mut self.windows[index];
            if let Some(action) = window.app.mouse(x, y, rect, theme) {
                actions.push(action);
            }
            return actions;
        }

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
        self.hover_control = None;
        if let Some(id) = self.window_at(x, y) {
            if let Some(window) = self.windows.iter().find(|w| w.id == id) {
                if !window.fullscreen && y < window.rect.y + TITLE_HEIGHT {
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
                    window.transition = None;
                    window.state = WindowState::Normal;
                    window.rect.x = (x - dx).clamp(-window.rect.w + 120, work_area.right() - 120);
                    window.rect.y = (y - dy).clamp(top_bar + 2, work_area.bottom() - 40);
                }
                if x <= SNAP_EDGE {
                    self.snap_preview = Some(Rect::new(
                        work_area.x,
                        work_area.y,
                        work_area.w / 2,
                        work_area.h,
                    ));
                } else if x >= work_area.right() - SNAP_EDGE {
                    self.snap_preview = Some(Rect::new(
                        work_area.x + work_area.w / 2,
                        work_area.y,
                        work_area.w / 2,
                        work_area.h,
                    ));
                } else if y <= top_bar + 4 {
                    self.snap_preview = Some(work_area.inset(8));
                } else {
                    self.snap_preview = None;
                }
            }
            Some(Interaction::Resize {
                id,
                edges,
                start_x,
                start_y,
                rect,
            }) => {
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
                    new_rect.y = (rect.y + dy)
                        .min(rect.bottom() - MIN_HEIGHT)
                        .max(top_bar + 2);
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
                    window.transition = None;
                    window.state = WindowState::Normal;
                }
            }
            None => {}
        }
    }

    pub fn mouse_up(&mut self, x: i32, y: i32, work_area: Rect) {
        if let Some(Interaction::Drag { id, .. }) = self.interaction {
            if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                window.normal = window.rect;
            }
            let mut snap = 0u8;
            if x <= SNAP_EDGE {
                snap = 1;
            } else if x >= work_area.right() - SNAP_EDGE {
                snap = 2;
            } else if y <= 34 {
                snap = 3;
            }
            if snap == 3 {
                self.toggle_maximize(id, work_area);
            } else if snap != 0 {
                let target = if snap == 1 {
                    Rect::new(work_area.x, work_area.y, work_area.w / 2, work_area.h)
                } else {
                    Rect::new(
                        work_area.x + work_area.w / 2,
                        work_area.y,
                        work_area.w / 2,
                        work_area.h,
                    )
                };
                self.snap_to(id, target, snap, true);
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

    pub fn route_action(
        &mut self,
        action: Action,
        theme: &Theme,
        work_area: Rect,
        target: Option<u64>,
    ) -> Option<Action> {
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

    /// Snapshot of visible windows for the overview and quick switcher.
    pub fn overview_items(&self) -> Vec<(u64, String, AppKind, usize, bool, bool, Rect)> {
        let mut items = Vec::new();
        for window in &self.windows {
            if window.state.is_closed() {
                continue;
            }
            items.push((
                window.id,
                window.title.clone(),
                window.kind(),
                window.desktop,
                window.state == WindowState::Minimized,
                window.fullscreen,
                window.normal,
            ));
        }
        items
    }

    pub fn window_rect(&self, id: u64, now: u64) -> Option<Rect> {
        self.windows
            .iter()
            .find(|window| window.id == id)
            .map(|window| window.current_rect(now))
    }

    pub fn focus_window(&mut self, id: u64) -> bool {
        let Some((desktop, state)) = self
            .windows
            .iter()
            .find(|window| window.id == id)
            .map(|window| (window.desktop, window.state))
        else {
            return false;
        };
        if self.desktop != desktop {
            self.set_desktop(desktop);
        }
        if matches!(state, WindowState::Minimized | WindowState::Minimizing) {
            self.restore(id);
        } else {
            self.focus_id(id);
        }
        true
    }

    pub fn close_minimized(&mut self, id: u64) {
        self.close(id);
    }

    pub fn minimized_count(&self, kind: AppKind) -> usize {
        self.windows
            .iter()
            .filter(|window| {
                window.kind() == kind
                    && matches!(
                        window.state,
                        WindowState::Minimized | WindowState::Minimizing
                    )
            })
            .count()
    }

    pub fn default_size_for(kind: AppKind) -> (i32, i32) {
        default_size(kind)
    }
}

fn default_size(kind: AppKind) -> (i32, i32) {
    match kind {
        AppKind::Terminal => (720, 440),
        AppKind::Files => (760, 470),
        AppKind::Settings => (820, 620),
        AppKind::Calculator => (360, 500),
        AppKind::Monitor => (780, 540),
        AppKind::Text => (720, 500),
        AppKind::Notes => (760, 480),
        AppKind::Calendar => (660, 500),
        AppKind::Photos => (740, 520),
        AppKind::Network => (860, 560),
    }
}
