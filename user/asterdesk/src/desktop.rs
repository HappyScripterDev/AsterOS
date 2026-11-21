//! Aster Desktop shell - a light, macOS-inspired desktop in ring 3.

//!

//! The shell owns the framebuffer and composes a menu bar, dock, Spotlight

//! launcher, windows, Control Centre and notifications. It is deliberately

//! light-only: there is no dark mode or theme switch in this build.

use alloc::format;

use alloc::string::{String, ToString};

use alloc::vec;

use alloc::vec::Vec;

use aster_abi::{mods, InputEvent, InputKind};

use crate::apps::{Action, App, AppKind};

use crate::font_data::{BODY, BODY_MEDIUM, CAPTION, HEADLINE, SMALL, TITLE};

use crate::gfx::{self, Color, FloatMath, Rect, Surface};
use crate::glass;
use crate::material::{self, Material};

use crate::logo_data;

use aster_motion::Easing;

use crate::render::{DirtyRegions, FramePacer, FrameStats, QualityMode};
use crate::services;
use crate::settings;

use crate::sys;

use crate::theme::{self, Theme, DOCK_GAP, DOCK_HEIGHT, TOP_BAR_HEIGHT};

use crate::ui::{self, Icon};

use crate::wm::WindowManager;

const DOCK_ICON: i32 = 50;

const DOCK_SLOT: i32 = 62;

const CONTROL_WIDTH: i32 = 322;

const CONTROL_HEIGHT: i32 = 336;

const MENU_WIDTH: i32 = 252;

struct Notification {
    title: String,

    body: String,

    created: u64,

    read: bool,
}

#[derive(Clone, Copy)]
struct WorkspaceAnim {
    from: usize,
    to: usize,
    start: u64,
    duration: u64,
    direction: i32,
}

struct Launcher {
    query: String,

    selected: usize,
}

struct OverviewItem {
    id: u64,
    title: String,
    kind: AppKind,
    desktop: usize,
    minimized: bool,
    fullscreen: bool,
    rect: Rect,
    pixels: Vec<u32>,
    source_width: i32,
    source_height: i32,
    card: Rect,
    scaled: Vec<u32>,
    scaled_width: i32,
    scaled_height: i32,
}

struct Overview {
    items: Vec<OverviewItem>,
    selected: usize,
    started: u64,
    closing: Option<u64>,
}

struct SwitcherItem {
    id: u64,
    title: String,
    kind: AppKind,
    desktop: usize,
    minimized: bool,
    fullscreen: bool,
    preview: Vec<u32>,
}

struct Switcher {
    items: Vec<SwitcherItem>,
    selected: usize,
    started: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]

enum MenuKind {
    App,

    File,

    Edit,

    View,

    Window,

    Help,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PowerPrompt {
    Shutdown,
    Restart,
}

#[derive(Clone, Copy, PartialEq, Eq)]

enum MenuAction {
    About,

    Settings,

    CloseWindow,

    Minimize,

    Zoom,

    Fullscreen,

    NextWindow,

    Launcher,

    Shortcuts,

    ShutDown,

    Restart,

    Desktop(usize),

    Notifications,
}

#[derive(Clone, Copy)]

struct MenuItem {
    label: &'static str,

    shortcut: &'static str,

    action: Option<MenuAction>,

    disabled: bool,

    checked: bool,

    separator: bool,
}

impl MenuItem {
    const fn item(
        label: &'static str,
        shortcut: &'static str,
        action: MenuAction,
        disabled: bool,
    ) -> Self {
        Self {
            label,

            shortcut,

            action: Some(action),

            disabled,

            checked: false,

            separator: false,
        }
    }

    const fn separator() -> Self {
        Self {
            label: "",

            shortcut: "",

            action: None,

            disabled: true,

            checked: false,

            separator: true,
        }
    }
}

pub struct Desktop {
    surface: Surface,

    framebuffer: *mut u32,

    fb_stride: u32,

    fb_format: u32,

    width: i32,

    height: i32,

    theme: Theme,

    saved_theme: Option<Theme>,

    wallpaper: Vec<u32>,

    blurred: Vec<u32>,

    dock_shadow: Vec<u8>,

    dock_shadow_size: (i32, i32),

    dock_visibility: f32,

    dock_last_update: u64,

    wm: WindowManager,

    mouse_x: i32,

    mouse_y: i32,

    launcher: Option<Launcher>,

    control_center: bool,

    notif_panel: bool,

    power_prompt: Option<PowerPrompt>,

    open_menu: Option<MenuKind>,

    notifications: Vec<Notification>,

    dock_hover: Option<usize>,

    dragging_brightness: bool,

    clock_text: String,

    last_clock: u64,

    net: Option<aster_abi::NetStatus>,

    last_net: u64,

    started: u64,

    frames: u64,

    frame_accum: u64,

    services: services::Manager,

    dirty: DirtyRegions,

    stats: FrameStats,

    pacer: FramePacer,

    needs_full: bool,

    last_cursor: Rect,

    last_clock_text: String,

    last_net_code: u8,

    last_control: bool,

    last_notif_panel: bool,

    last_menu: Option<MenuKind>,

    last_menu_rect: Rect,

    last_toast_count: usize,

    last_launcher: bool,

    last_live: u64,

    last_loop_start: u64,

    workspace_anim: Option<WorkspaceAnim>,

    last_workspace_switch: u64,

    overview: Option<Overview>,

    switcher: Option<Switcher>,

    menu_started: u64,

    menu_fade: Option<(MenuKind, u64)>,

    glass_small: Vec<u32>,

    glass_scratch: Vec<u32>,
}

impl Desktop {
    pub fn new(
        width: i32,

        height: i32,

        framebuffer: *mut u32,

        fb_stride: u32,

        fb_format: u32,
    ) -> Option<Self> {
        let surface = Surface::new(width, height, fb_format)?;

        let dock_width = DOCK_SLOT * AppKind::ALL.len() as i32 + 24;

        let mut desktop = Self {
            surface,

            framebuffer,

            fb_stride,

            fb_format,

            width,

            height,

            theme: Theme::default(),

            saved_theme: None,

            wallpaper: vec![0; (width * height) as usize],

            blurred: Vec::new(),

            dock_shadow: gfx::shadow_mask(dock_width, DOCK_HEIGHT, 20, 24, 26),

            dock_shadow_size: (dock_width, DOCK_HEIGHT),

            dock_visibility: 1.0,

            dock_last_update: 0,

            wm: WindowManager::new(),

            mouse_x: width / 2,

            mouse_y: height / 2,

            launcher: None,

            control_center: false,

            notif_panel: false,

            power_prompt: None,

            open_menu: None,

            notifications: Vec::new(),

            dock_hover: None,

            dragging_brightness: false,

            clock_text: String::new(),

            last_clock: 0,

            net: None,

            last_net: 0,

            started: sys::time_ms(),

            frames: 0,

            frame_accum: 0,

            services: services::Manager::new(),

            dirty: DirtyRegions::new(width, height),

            stats: FrameStats::default(),

            pacer: FramePacer::new(60),

            needs_full: true,

            last_cursor: Rect::new(0, 0, 0, 0),

            last_clock_text: String::new(),

            last_net_code: 255,

            last_control: false,

            last_notif_panel: false,

            last_menu: None,

            last_menu_rect: Rect::new(0, 0, 0, 0),

            last_toast_count: 0,

            last_launcher: false,

            last_live: 0,

            last_loop_start: 0,

            workspace_anim: None,

            last_workspace_switch: 0,

            overview: None,

            switcher: None,

            menu_started: 0,

            menu_fade: None,

            glass_small: vec![0u32; ((width * height / 4).max(4096)) as usize],

            glass_scratch: vec![0u32; ((width * height / 4).max(4096)) as usize],
        };

        desktop.services.start();

        desktop.load_preferences();

        desktop.rebuild_wallpaper();

        desktop.update_clock(true);

        desktop.notify(
            "Welcome to Aster OS",
            "Press Ctrl+Space for Spotlight, or use the Dock to open an app.",
        );

        Some(desktop)
    }

    fn work_area(&self) -> Rect {
        let dock_h = self.dock_height();
        let dock_w = self.dock_width();
        let reserve = !self.theme.dock_autohide;
        let (left, right) = if reserve && self.theme.dock_position == 1 {
            (dock_w + DOCK_GAP + 6, self.width - 6)
        } else if reserve && self.theme.dock_position == 2 {
            (6, self.width - dock_w - DOCK_GAP - 6)
        } else {
            (6, self.width - 6)
        };
        let bottom = if reserve && self.theme.dock_position == 0 {
            self.height - dock_h - DOCK_GAP - 6
        } else {
            self.height - 6
        };
        Rect::new(
            left,
            TOP_BAR_HEIGHT + 6,
            (right - left).max(200),
            (bottom - TOP_BAR_HEIGHT - 6).max(160),
        )
    }

    fn dock_icon_size(&self) -> i32 {
        (DOCK_ICON as f32 * self.theme.dock_size.clamp(0.7, 1.3)) as i32
    }

    fn dock_slot_size(&self) -> i32 {
        self.dock_icon_size() + 12
    }

    fn dock_height(&self) -> i32 {
        self.dock_icon_size() + 18
    }

    fn dock_width(&self) -> i32 {
        self.dock_slot_size() * AppKind::ALL.len() as i32 + 24
    }

    /// Base rectangle before auto-hide offset. `dock_rect` applies the
    /// visibility offset used for drawing and hit testing.
    fn dock_base_rect(&self) -> Rect {
        match self.theme.dock_position {
            1 => {
                let width = self.dock_height();
                let height = self.dock_width();
                Rect::new(DOCK_GAP, (self.height - height) / 2, width, height)
            }
            2 => {
                let width = self.dock_height();
                let height = self.dock_width();
                Rect::new(
                    self.width - width - DOCK_GAP,
                    (self.height - height) / 2,
                    width,
                    height,
                )
            }
            _ => {
                let width = self.dock_width();
                let height = self.dock_height();
                Rect::new(
                    (self.width - width) / 2,
                    self.height - height - DOCK_GAP,
                    width,
                    height,
                )
            }
        }
    }

    fn dock_rect(&self) -> Rect {
        let mut rect = self.dock_base_rect();
        if self.theme.dock_autohide && self.dock_visibility < 0.999 {
            let hidden = ((1.0 - self.dock_visibility) * (rect.h + 12) as f32) as i32;
            match self.theme.dock_position {
                1 => rect.x -= rect.w + 4 - hidden,
                2 => rect.x += hidden,
                _ => rect.y += hidden,
            }
        }
        rect
    }

    fn dock_hit_rect(&self) -> Rect {
        let rect = self.dock_rect();
        match self.theme.dock_position {
            1 => Rect::new(0, rect.y, rect.w + 6, rect.h),
            2 => Rect::new(rect.x - 6, rect.y, rect.w + 6, rect.h),
            _ => Rect::new(rect.x, rect.y - 4, rect.w, rect.h + 6),
        }
    }

    fn update_dock_visibility(&mut self) {
        let now = sys::time_ms();
        let dt = now.saturating_sub(self.dock_last_update).min(50) as f32;
        self.dock_last_update = now;
        if !self.theme.dock_autohide {
            self.dock_visibility = 1.0;
            return;
        }
        let base = self.dock_base_rect();
        let near_edge = match self.theme.dock_position {
            1 => self.mouse_x <= 10,
            2 => self.mouse_x >= self.width - 10,
            _ => self.mouse_y >= self.height - 10,
        };
        let hovering = base.inset(8).contains(self.mouse_x, self.mouse_y);
        let target = if near_edge || hovering || self.dragging_brightness {
            1.0
        } else {
            0.0
        };
        if self.theme.reduce_motion || !self.theme.dock_animations {
            if (self.dock_visibility - target).abs() > 0.001 {
                self.dock_visibility = target;
                self.dirty.add(self.dock_base_rect().inset(-32));
            }
            return;
        }
        let previous = self.dock_visibility;
        let step = dt / 140.0;
        if self.dock_visibility < target {
            self.dock_visibility = (self.dock_visibility + step).min(1.0);
        } else {
            self.dock_visibility = (self.dock_visibility - step).max(0.0);
        }
        if (previous - self.dock_visibility).abs() > 0.001 {
            self.dirty.add(self.dock_base_rect().inset(-32));
        }
    }

    fn dock_anchors(&self) -> [(AppKind, (i32, i32)); 10] {
        let mut anchors = [(AppKind::Terminal, (0, 0)); 10];
        for (index, kind) in AppKind::ALL.iter().enumerate() {
            let icon = self.dock_icon_rect(index, 1.0);
            anchors[index] = (*kind, icon.center());
        }
        anchors
    }

    fn launcher_rect(&self) -> Rect {
        let rows = self
            .launcher
            .as_ref()
            .map(|launcher| Self::filter_apps(&launcher.query).len().clamp(1, 6))
            .unwrap_or(6);

        let height = 84 + rows as i32 * 42 + 26;

        Rect::new((self.width - 660) / 2, 96, 660, height)
    }

    fn control_rect(&self) -> Rect {
        Rect::new(
            self.width - CONTROL_WIDTH - 12,
            TOP_BAR_HEIGHT + 8,
            CONTROL_WIDTH,
            CONTROL_HEIGHT,
        )
    }

    fn notif_rect(&self) -> Rect {
        let rows = self.notifications.len().min(5) as i32;
        let height = if self.notifications.is_empty() {
            150
        } else {
            96 + rows * 58
        };
        Rect::new(self.width - 372, TOP_BAR_HEIGHT + 8, 356, height)
    }

    fn app_slot(&self, index: usize) -> Rect {
        let dock = self.dock_rect();
        let slot = self.dock_slot_size();
        match self.theme.dock_position {
            1 | 2 => Rect::new(dock.x, dock.y + 12 + index as i32 * slot, dock.w, slot),
            _ => Rect::new(dock.x + 12 + index as i32 * slot, dock.y, slot, dock.h),
        }
    }

    fn dock_icon_rect(&self, index: usize, scale: f32) -> Rect {
        let slot = self.app_slot(index);
        let size = (self.dock_icon_size() as f32 * scale) as i32;
        Rect::new(
            slot.x + (slot.w - size) / 2,
            slot.y + (slot.h - size) / 2,
            size,
            size,
        )
    }

    fn app_name(&self) -> String {
        self.wm
            .focused()
            .map(|window| window.kind().title().to_string())
            .unwrap_or_else(|| "Aster".to_string())
    }

    fn load_preferences(&mut self) {
        // Prefer the settings service; fall back to direct file I/O when it is
        // unavailable so a service failure cannot lock the user out.
        let mut service_buffer = [0u8; 256];
        if let Some(length) = self.services.get_settings(&mut service_buffer) {
            if let Ok(text) = core::str::from_utf8(&service_buffer[..length]) {
                let loaded = settings::parse(text);
                if !loaded.corrupt {
                    loaded.settings.apply_to(&mut self.theme);
                    self.saved_theme = Some(self.theme);
                    if loaded.needs_save {
                        self.saved_theme = None;
                        self.save_preferences();
                    }
                    return;
                }
            }
        }

        let mut data = sys::read_file(settings::PATH);
        let mut from_backup = false;
        if data.is_none() {
            data = sys::read_file(settings::BACKUP);
            from_backup = data.is_some();
        }
        let Some(bytes) = data else {
            // No settings yet: write a complete, versioned default file.
            self.saved_theme = None;
            self.save_preferences();
            return;
        };

        let Ok(text) = core::str::from_utf8(&bytes) else {
            sys::log(b"settings: invalid UTF-8, restoring defaults\n");
            let _ = sys::unlink(settings::BAD);
            let _ = sys::rename(settings::PATH, settings::BAD);
            self.theme = Theme::default();
            self.saved_theme = None;
            self.save_preferences();
            return;
        };

        let loaded = settings::parse(text);
        loaded.settings.apply_to(&mut self.theme);

        if loaded.corrupt {
            sys::log(b"settings: corrupt file, preserving it as SETTINGS.BAD\n");
            let _ = sys::unlink(settings::BAD);
            let _ = sys::rename(settings::PATH, settings::BAD);
            self.theme = Theme::default();
            self.saved_theme = None;
            self.save_preferences();
        } else if loaded.needs_save || from_backup {
            // Migrate an older file or replace a recovered backup.
            self.saved_theme = None;
            self.save_preferences();
        } else {
            self.saved_theme = Some(self.theme);
        }
    }

    fn save_preferences(&mut self) {
        if self.saved_theme == Some(self.theme) {
            return;
        }

        let content = settings::Settings::from_theme(&self.theme).serialize();

        if self.services.set_settings(&content) {
            self.saved_theme = Some(self.theme);
            return;
        }

        sys::log(b"settings: service unavailable, using direct I/O\n");

        // Write a temporary file, flush it, keep the previous file as a
        // backup, then rename the temporary file over the live one.
        if !sys::write_file(settings::TEMP, content.as_bytes()) {
            sys::log(b"settings: cannot write the temporary file\n");
            return;
        }
        let _ = sys::sync();

        if sys::stat(settings::PATH).is_some() {
            let _ = sys::unlink(settings::BACKUP);
            if sys::rename(settings::PATH, settings::BACKUP) < 0 {
                sys::log(b"settings: backup rename failed\n");
            }
        }

        let mut stored = sys::rename(settings::TEMP, settings::PATH) >= 0;
        if !stored {
            // Some FAT paths cannot rename; fall back to a direct write so the
            // user still gets persistence, but log the weaker guarantee.
            stored = sys::write_file(settings::PATH, content.as_bytes());
            let _ = sys::unlink(settings::TEMP);
        }
        let _ = sys::sync();

        if stored {
            self.saved_theme = Some(self.theme);
        } else {
            sys::log(b"settings: save failed\n");
        }
    }

    fn rebuild_wallpaper(&mut self) {
        let small_w = 480;

        let small_h = 300;

        let mut small = vec![0u32; (small_w * small_h) as usize];

        theme::paint_wallpaper(&mut small, small_w, small_h, self.theme.wallpaper);

        gfx::upscale_nearest(
            &small,
            small_w,
            small_h,
            &mut self.wallpaper,
            self.width,
            self.height,
        );

        let blur_w = 320;

        let blur_h = 200;

        let source = gfx::downsample(&self.wallpaper, self.width, self.height, blur_w, blur_h);

        let small_blur = gfx::box_blur(&source, blur_w, blur_h, 4);

        let mut blurred = vec![0u32; (self.width * self.height) as usize];

        gfx::upscale_nearest(
            &small_blur,
            blur_w,
            blur_h,
            &mut blurred,
            self.width,
            self.height,
        );

        self.blurred = blurred;

        self.needs_full = true;

        sys::log(b"desktop: wallpaper ready");
    }

    fn notify(&mut self, title: &str, body: &str) {
        self.notifications.push(Notification {
            title: title.to_string(),

            body: body.to_string(),

            created: sys::time_ms(),

            read: false,
        });

        if self.notifications.len() > 64 {
            self.notifications.remove(0);
        }
    }

    fn update_clock(&mut self, force: bool) {
        let now = sys::time_ms();

        if !force && now.saturating_sub(self.last_clock) < 1000 {
            return;
        }

        self.last_clock = now;

        if now.saturating_sub(self.last_net) >= 1000 || self.net.is_none() {
            self.net = sys::net_status();
            self.last_net = now;
        }

        if let Some(time) = sys::rtc() {
            let weekday = match time.weekday {
                1 => "Sun",

                2 => "Mon",

                3 => "Tue",

                4 => "Wed",

                5 => "Thu",

                6 => "Fri",

                _ => "Sat",
            };

            let hour12 = if time.hour % 12 == 0 {
                12
            } else {
                time.hour % 12
            };

            let meridiem = if time.hour < 12 { "AM" } else { "PM" };

            self.clock_text = format!(
                "{} {} {} {}:{:02} {}",
                weekday,
                month_short(time.month),
                time.day,
                hour12,
                time.minute,
                meridiem
            );
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            let frame_start = sys::time_ms();
            let interval = if self.last_loop_start == 0 {
                0
            } else {
                frame_start.saturating_sub(self.last_loop_start)
            };
            self.last_loop_start = frame_start;
            let effective_quality = if self.theme.quality == QualityMode::Adaptive.code() {
                if self.stats.average_frame_ms > 26.0 {
                    QualityMode::Performance
                } else {
                    QualityMode::Balanced
                }
            } else {
                QualityMode::from_code(self.theme.quality)
            };
            self.dirty.set_quality(effective_quality);
            let target = if self.theme.frame_limit == 0 {
                60
            } else {
                self.theme.frame_limit as u32
            };
            self.pacer.set_target(target);

            let mut events: [InputEvent; 64] = unsafe { core::mem::zeroed() };
            let count = sys::poll_input(&mut events);
            let after_input = sys::time_ms();
            for event in &events[..count] {
                self.handle_event(event);
            }

            self.update_clock(false);
            self.update_dock_visibility();
            self.wm.set_dock_anchors(self.dock_anchors());
            self.wm.set_screen(Rect::new(0, 0, self.width, self.height));
            self.wm.tick(&self.theme);
            self.services.tick();

            let now = sys::time_ms();
            self.collect_dirty(now);

            let mut draw_ms = 0u64;
            let mut present_ms = 0u64;
            if !self.dirty.is_empty() {
                let regions = self.dirty_regions();
                let draw_start = sys::time_ms();
                self.paint(now, &regions.0[..regions.1]);
                draw_ms = sys::time_ms().saturating_sub(draw_start);

                let present_start = sys::time_ms();
                self.present_regions(&regions.0[..regions.1]);
                present_ms = sys::time_ms().saturating_sub(present_start);

                let animations = if self.wm.is_animating(now) { 1 } else { 0 }
                    + if self.dock_visibility < 0.999
                        || self.dock_visibility > 0.001 && self.theme.dock_autohide
                    {
                        1
                    } else {
                        0
                    };
                let active = self
                    .wm
                    .windows
                    .iter()
                    .filter(|w| !w.state.is_closed())
                    .count() as u32;
                let measured = if interval > 0 {
                    interval
                } else {
                    sys::time_ms().saturating_sub(frame_start)
                };
                self.stats.record(
                    measured,
                    after_input.saturating_sub(frame_start),
                    draw_ms,
                    present_ms,
                    &self.dirty,
                    animations,
                    active,
                );
                crate::render::publish(&self.stats);
                self.dirty.clear();
                self.needs_full = false;
            } else {
                let measured = if interval > 0 {
                    interval
                } else {
                    sys::time_ms().saturating_sub(frame_start)
                };
                self.stats.frames += 1;
                self.stats.skipped_frames += 1;
                self.stats.last_frame_ms = measured;
                self.stats.last_draw_ms = 0;
                self.stats.last_present_ms = 0;
                self.stats.dirty_regions = 0;
                self.stats.dirty_pixels = 0;
                self.stats.worst_frame_ms = self.stats.worst_frame_ms.max(measured);
                self.stats.average_frame_ms = if self.stats.frames == 1 {
                    measured as f32
                } else {
                    self.stats.average_frame_ms * 0.95 + measured as f32 * 0.05
                };
                crate::render::publish(&self.stats);
            }

            let elapsed = sys::time_ms().saturating_sub(frame_start);
            if self.frames <= 10 || self.stats.frames % 60 == 0 {
                let average = if self.stats.average_frame_ms > 0.0 {
                    self.stats.average_frame_ms
                } else {
                    elapsed as f32
                };
                sys::log(
                    format!(
                        "desktop: frame {} at {} ms, {:.1} ms avg ({:.1} fps, input {} draw {} present {} dirty {} px, full {}, skipped {})",
                        self.stats.frames,
                        sys::time_ms(),
                        average,
                        if average > 0.0 { 1000.0 / average } else { 0.0 },
                        after_input.saturating_sub(frame_start),
                        draw_ms,
                        present_ms,
                        self.stats.dirty_pixels,
                        self.stats.full_redraws,
                        self.stats.skipped_frames
                    )
                    .as_bytes(),
                );
            }
            self.frames += 1;

            let budget = self.pacer.budget_ms();
            if self.theme.frame_limit != 0 && elapsed < budget {
                sys::sleep_ms(budget - elapsed);
            }
        }
    }

    fn cursor_rect(x: i32, y: i32) -> Rect {
        Rect::new(x - 2, y - 2, 18, 24)
    }

    fn mark_window(&mut self, x: i32, y: i32, now: u64) {
        if let Some(id) = self.wm.window_at(x, y) {
            if let Some(rect) = self.wm.window_rect(id, now) {
                self.dirty.add(rect.inset(-2));
            }
        }
    }

    fn toast_area(&self) -> Rect {
        let height = (self.notifications.len().min(3) as i32) * 92 + 12;
        Rect::new(self.width - 356, TOP_BAR_HEIGHT + 8, 356, height.max(96))
    }

    fn collect_dirty(&mut self, now: u64) {
        if self.wm.force_full {
            self.wm.force_full = false;
            self.dirty.add_all();
        }
        if let Some(animation) = self.workspace_anim {
            if now.saturating_sub(animation.start) >= animation.duration {
                self.workspace_anim = None;
                self.needs_full = true;
            }
        }
        if self.workspace_anim.is_some() {
            self.dirty.add_all();
        }
        if let Some(overview) = &self.overview {
            if let Some(close) = overview.closing {
                if now.saturating_sub(close) >= 150 {
                    self.overview = None;
                    self.needs_full = true;
                }
            }
        }
        if let Some(overview) = &self.overview {
            let opening = now.saturating_sub(overview.started) < 420;
            let closing = overview.closing.is_some();
            if opening || closing || self.needs_full {
                self.dirty.add_all();
            }
        }
        if self.switcher.is_some() {
            self.dirty.add_all();
        }
        if self.needs_full || self.launcher.is_some() || self.last_launcher {
            self.dirty.add_all();
        } else {
            let regions = self.wm.animation_regions(now);
            for rect in regions {
                self.dirty.add(rect);
            }

            if now.saturating_sub(self.last_live) >= 500 {
                for window in &self.wm.windows {
                    if window.desktop == self.wm.desktop
                        && !window.state.is_closed()
                        && window.state != crate::wm::WindowState::Minimized
                    {
                        self.dirty.add(window.current_rect(now).inset(-2));
                    }
                }
                self.last_live = now;
            }

            if self.control_center {
                self.dirty.add(self.control_rect().inset(-8));
            }
            if self.last_control && !self.control_center {
                self.dirty.add(self.control_rect().inset(-8));
            }

            if self.notif_panel {
                self.dirty.add(self.notif_rect().inset(-8));
            }
            if self.last_notif_panel && !self.notif_panel {
                self.dirty.add(self.notif_rect().inset(-8));
            }

            if self.last_menu != self.open_menu {
                if self.open_menu.is_some() {
                    self.menu_started = now;
                    self.menu_fade = None;
                } else if let Some(previous) = self.last_menu {
                    self.menu_fade = Some((previous, now));
                }
            }
            if let Some(menu) = self.open_menu {
                let (rect, _) = self.menu_geometry(menu);
                self.dirty.add(rect.inset(-4));
                self.last_menu_rect = rect;
            }
            if let Some((menu, start)) = self.menu_fade {
                if now.saturating_sub(start) >= 120 {
                    self.menu_fade = None;
                } else {
                    let (rect, _) = self.menu_geometry(menu);
                    self.dirty.add(rect.inset(-4));
                }
            }
            if self.last_menu.is_some() && self.open_menu != self.last_menu {
                self.dirty.add(self.last_menu_rect.inset(-4));
            }

            let active_toasts = self
                .notifications
                .iter()
                .filter(|notification| {
                    !notification.read && now.saturating_sub(notification.created) <= 6000
                })
                .count();
            if active_toasts > 0 || self.last_toast_count > 0 {
                self.dirty.add(self.toast_area());
            }

            if now.saturating_sub(self.last_workspace_switch) <= 1100 {
                self.dirty.add(Rect::new(
                    self.width / 2 - 40,
                    self.dock_rect().y - 34,
                    80,
                    28,
                ));
            }

            if now.saturating_sub(self.last_workspace_switch) <= 1100 {
                self.dirty.add(Rect::new(
                    self.width / 2 - 40,
                    self.dock_rect().y - 34,
                    80,
                    28,
                ));
            }

            if self.clock_text != self.last_clock_text {
                self.dirty.add(Rect::new(0, 0, self.width, TOP_BAR_HEIGHT));
            }
            let net_code = self
                .net
                .map(|status| {
                    (status.present != 0) as u8
                        | ((status.link_up != 0) as u8) << 1
                        | ((status.configured != 0) as u8) << 2
                })
                .unwrap_or(0);
            if net_code != self.last_net_code {
                self.dirty.add(Rect::new(0, 0, self.width, TOP_BAR_HEIGHT));
            }

            if self.dirty.is_empty() && self.wm.is_animating(now) {
                if let Some(bounds) = self.wm.visible_bounds() {
                    self.dirty.add(bounds);
                }
            }
        }

        self.last_launcher = self.launcher.is_some();
        self.last_control = self.control_center;
        self.last_notif_panel = self.notif_panel;
        self.last_menu = self.open_menu;
        self.last_clock_text = self.clock_text.clone();
        self.last_net_code = self
            .net
            .map(|status| {
                (status.present != 0) as u8
                    | ((status.link_up != 0) as u8) << 1
                    | ((status.configured != 0) as u8) << 2
            })
            .unwrap_or(0);
        self.last_toast_count = self
            .notifications
            .iter()
            .filter(|notification| {
                !notification.read && now.saturating_sub(notification.created) <= 6000
            })
            .count();
    }

    fn dirty_regions(&self) -> ([Rect; 8], usize) {
        let mut regions = [Rect::new(0, 0, 0, 0); 8];
        if self.dirty.is_full() {
            regions[0] = Rect::new(0, 0, self.width, self.height);
            return (regions, 1);
        }
        let source = self.dirty.regions();
        let count = source.len().min(8);
        for (index, rect) in source.iter().take(count).enumerate() {
            regions[index] = *rect;
        }
        (regions, count)
    }

    fn copy_wallpaper(&mut self, rect: Rect) {
        let rect = rect.intersect(Rect::new(0, 0, self.width, self.height));
        if rect.is_empty() {
            return;
        }
        unsafe {
            for y in rect.y..rect.bottom() {
                core::ptr::copy_nonoverlapping(
                    self.wallpaper
                        .as_ptr()
                        .add((y * self.width + rect.x) as usize),
                    self.surface.pixels.add((y * self.width + rect.x) as usize),
                    rect.w as usize,
                );
            }
        }
    }

    fn paint(&mut self, now: u64, regions: &[Rect]) {
        let mut theme = self.theme;
        if theme.quality == QualityMode::Adaptive.code() {
            theme.quality = if self.stats.average_frame_ms > 26.0 {
                QualityMode::Performance.code()
            } else {
                QualityMode::Balanced.code()
            };
        }
        let full = regions.len() == 1 && regions[0].w == self.width && regions[0].h == self.height;
        let screen = Rect::new(0, 0, self.width, self.height);

        for region in regions {
            let clip = (*region).intersect(screen);
            if clip.is_empty() {
                continue;
            }
            self.surface.set_clip(clip);
            self.surface.opacity = 255;
            if full {
                if let Some((progress, animation)) = self.workspace_progress(now) {
                    let width = self.width as f32;
                    let left_offset = -animation.direction as f32 * progress * width;
                    let right_offset = animation.direction as f32 * (1.0 - progress) * width;
                    self.copy_wallpaper_offset(left_offset as i32);
                    self.wm.draw_for_desktop(
                        &mut self.surface,
                        &theme,
                        now,
                        clip,
                        TOP_BAR_HEIGHT,
                        animation.from,
                        left_offset as i32,
                    );
                    self.copy_wallpaper_offset(right_offset as i32);
                    self.wm.draw_for_desktop(
                        &mut self.surface,
                        &theme,
                        now,
                        clip,
                        TOP_BAR_HEIGHT,
                        animation.to,
                        right_offset as i32,
                    );
                } else {
                    self.copy_wallpaper(clip);
                    self.wm
                        .draw(&mut self.surface, &theme, now, clip, TOP_BAR_HEIGHT);
                }
            } else {
                self.copy_wallpaper(clip);
                self.wm
                    .draw(&mut self.surface, &theme, now, clip, TOP_BAR_HEIGHT);
            }

            if full && self.launcher.is_some() {
                self.draw_launcher();
            }
            if !self.wm.fullscreen_active() {
                self.draw_dock();
                self.draw_top_bar();
                self.draw_workspace_indicator();
            }
            if self.control_center && !self.wm.fullscreen_active() {
                self.draw_control_center();
            }
            if self.notif_panel {
                self.draw_notifications_panel();
            }
            if self.open_menu.is_some() {
                self.draw_menu();
            }
            self.draw_menu_fade(now);
            if !self.notifications.is_empty() && self.launcher.is_none() {
                self.draw_toasts();
            }
            if self.overview.is_some() {
                self.draw_overview(now);
            }
            if self.switcher.is_some() {
                self.draw_switcher(now);
            }
            if let Some(prompt) = self.power_prompt {
                self.draw_power_prompt(prompt);
            }
            self.draw_cursor();

            if self.theme.brightness < 0.999 {
                let alpha = ((1.0 - self.theme.brightness) * 150.0) as u8;
                self.surface.fill_rect(
                    Rect::new(0, 0, self.width, self.height),
                    Color::rgba(0, 0, 0, alpha),
                );
            }
        }
        self.surface.set_clip(screen);
        self.surface.opacity = 255;
    }

    fn present_regions(&self, regions: &[Rect]) {
        for region in regions {
            self.surface
                .present_region(self.framebuffer, self.fb_stride, self.fb_format, *region);
        }
    }

    // -----------------------------------------------------------------

    // Input

    // -----------------------------------------------------------------

    fn handle_event(&mut self, event: &InputEvent) {
        match event.kind {
            k if k == InputKind::MouseMove as u32 => {
                if self.overview.is_some() {
                    let selected = {
                        let Some(overview) = &self.overview else {
                            return;
                        };
                        let layout = self.overview_layout(overview);
                        layout
                            .iter()
                            .position(|rect| rect.contains(event.x, event.y))
                    };
                    if let (Some(index), Some(overview)) = (selected, &mut self.overview) {
                        if overview.selected != index {
                            overview.selected = index;
                            self.needs_full = true;
                        }
                    }
                }
                let old_x = self.mouse_x;
                let old_y = self.mouse_y;
                let now = sys::time_ms();
                self.dirty.add(Self::cursor_rect(old_x, old_y));
                self.mark_window(old_x, old_y, now);
                if old_y < TOP_BAR_HEIGHT + 4 || event.y < TOP_BAR_HEIGHT + 4 {
                    self.dirty.add(Rect::new(0, 0, self.width, TOP_BAR_HEIGHT));
                }
                if old_y > self.height - 150 || event.y > self.height - 150 {
                    self.dirty.add(self.dock_hit_rect());
                }
                self.mouse_x = event.x.clamp(0, self.width - 1);

                self.mouse_y = event.y.clamp(0, self.height - 1);

                if self.dragging_brightness {
                    let panel = self.control_rect();

                    let value = ((self.mouse_x - (panel.x + 18)) as f32 / (panel.w - 36) as f32)
                        .clamp(0.0, 1.0);

                    self.theme.brightness = value.max(0.3);
                    self.needs_full = true;
                }

                let work = self.work_area();
                if let Some(id) = self.wm.interaction_window() {
                    if let Some(rect) = self.wm.window_rect(id, now) {
                        self.dirty.add(rect.inset(-32));
                    }
                }

                self.wm
                    .mouse_move(self.mouse_x, self.mouse_y, work, TOP_BAR_HEIGHT);
                if let Some(id) = self.wm.interaction_window() {
                    if let Some(rect) = self.wm.window_rect(id, now) {
                        self.dirty.add(rect.inset(-32));
                    }
                }

                if self.open_menu.is_some() {
                    if let Some(kind) = self.menu_at(self.mouse_x, self.mouse_y) {
                        self.open_menu = Some(kind);
                    }
                }
                self.dirty
                    .add(Self::cursor_rect(self.mouse_x, self.mouse_y));
                self.mark_window(self.mouse_x, self.mouse_y, now);
            }

            k if k == InputKind::MouseButton as u32 => {
                if event.code == 0 {
                    if event.value == 1 {
                        self.mouse_press(self.mouse_x, self.mouse_y);
                    } else {
                        let work = self.work_area();
                        if let Some((id, direction)) =
                            self.wm.mouse_up(self.mouse_x, self.mouse_y, work)
                        {
                            let target = (self.wm.desktop as i32 + direction)
                                .clamp(0, self.wm.desktop_count as i32 - 1)
                                as usize;
                            if target != self.wm.desktop {
                                self.wm.move_to_desktop(id, target);
                                self.begin_workspace_jump(target);
                            }
                        }
                        self.dragging_brightness = false;
                        self.save_preferences();
                    }
                    let now = sys::time_ms();
                    self.mark_window(self.mouse_x, self.mouse_y, now);
                    self.dirty.add(Rect::new(0, 0, self.width, TOP_BAR_HEIGHT));
                    self.dirty.add(self.dock_hit_rect());
                    if self.control_center || self.notif_panel {
                        self.dirty.add(self.control_rect().inset(-8));
                    }
                }
            }

            k if k == InputKind::MouseWheel as u32 => {
                if self
                    .wm
                    .wheel(self.mouse_x, self.mouse_y, event.value, &self.theme)
                {
                    let now = sys::time_ms();
                    self.mark_window(self.mouse_x, self.mouse_y, now);
                }
            }

            k if k == InputKind::Key as u32 => {
                if event.value == 1 {
                    self.key_press(event);
                } else if self.switcher.is_some()
                    && (event.code == aster_abi::keys::LEFT_ALT
                        || event.code == aster_abi::keys::RIGHT_ALT)
                {
                    self.commit_switcher();
                }
                let now = sys::time_ms();
                if let Some(focus) = self.wm.focus {
                    if let Some(rect) = self.wm.window_rect(focus, now) {
                        self.dirty.add(rect.inset(-2));
                    }
                }
                if self.control_center || self.notif_panel {
                    self.dirty.add(self.control_rect().inset(-8));
                }
            }

            _ => {}
        }
    }

    fn open_overview(&mut self) {
        if self.overview.is_some() {
            self.close_overview();
            return;
        }
        self.switcher = None;
        let now = sys::time_ms();
        let mut items = Vec::new();
        for (id, title, kind, desktop, minimized, fullscreen, normal) in self.wm.overview_items() {
            let rect = if minimized {
                normal
            } else {
                self.wm.window_rect(id, now).unwrap_or(normal)
            };
            let (pixels, source_width, source_height) = if minimized {
                (Vec::new(), 0, 0)
            } else {
                match self.surface.copy_region(rect) {
                    Some((data, width, height)) => (data, width, height),
                    None => (Vec::new(), 0, 0),
                }
            };
            items.push(OverviewItem {
                id,
                title,
                kind,
                desktop,
                minimized,
                fullscreen,
                rect,
                pixels,
                source_width,
                source_height,
                card: Rect::new(0, 0, 0, 0),
                scaled: Vec::new(),
                scaled_width: 0,
                scaled_height: 0,
            });
        }
        let selected = items
            .iter()
            .position(|item| Some(item.id) == self.wm.focus)
            .unwrap_or(0);
        let mut overview = Overview {
            items,
            selected,
            started: now,
            closing: None,
        };
        let layout = self.overview_layout(&overview);
        for (index, item) in overview.items.iter_mut().enumerate() {
            item.card = layout[index];
            let preview = Rect::new(
                item.card.x + 6,
                item.card.y + 6,
                item.card.w - 12,
                item.card.h - 38,
            );
            if !item.minimized
                && item.source_width > 0
                && item.source_height > 0
                && preview.w > 0
                && preview.h > 0
            {
                let mut scaled = vec![0u32; (preview.w * preview.h) as usize];
                gfx::upscale_nearest(
                    &item.pixels,
                    item.source_width,
                    item.source_height,
                    &mut scaled,
                    preview.w,
                    preview.h,
                );
                item.scaled = scaled;
                item.scaled_width = preview.w;
                item.scaled_height = preview.h;
            }
        }
        self.overview = Some(overview);
        self.needs_full = true;
    }

    fn close_overview(&mut self) {
        if let Some(overview) = &mut self.overview {
            if overview.closing.is_none() {
                overview.closing = Some(sys::time_ms());
                self.needs_full = true;
            }
        }
    }

    fn finish_overview(&mut self, id: u64) {
        self.overview = None;
        self.wm.focus_window(id);
        self.needs_full = true;
    }

    fn overview_select(&mut self, id: u64) {
        self.overview = None;
        self.wm.focus_window(id);
        self.needs_full = true;
    }

    fn advance_overview(&mut self, delta: i32) {
        let Some(overview) = &mut self.overview else {
            return;
        };
        if overview.items.is_empty() {
            return;
        }
        let count = overview.items.len() as i32;
        overview.selected = ((overview.selected as i32 + delta).rem_euclid(count)) as usize;
        self.needs_full = true;
    }

    fn overview_layout(&self, overview: &Overview) -> Vec<Rect> {
        let count = overview.items.len().max(1);
        let columns = if count <= 2 {
            count
        } else if count <= 6 {
            2
        } else {
            3
        };
        let rows = (count + columns - 1) / columns;
        let area = Rect::new(
            56,
            TOP_BAR_HEIGHT + 44,
            self.width - 112,
            self.height - TOP_BAR_HEIGHT - 150,
        );
        let gap = 20;
        let cell_w = (area.w - (columns as i32 - 1) * gap) / columns as i32;
        let cell_h = (area.h - (rows as i32 - 1) * gap) / rows as i32;
        let mut rects = Vec::new();
        for index in 0..overview.items.len() {
            let row = index as i32 / columns as i32;
            let col = index as i32 % columns as i32;
            rects.push(Rect::new(
                area.x + col * (cell_w + gap),
                area.y + row * (cell_h + gap),
                cell_w,
                cell_h,
            ));
        }
        rects
    }

    fn draw_overview(&mut self, now: u64) {
        let Some(overview) = &self.overview else {
            return;
        };
        let progress = if let Some(close) = overview.closing {
            1.0 - (now.saturating_sub(close) as f32 / 150.0).clamp(0.0, 1.0)
        } else {
            (now.saturating_sub(overview.started) as f32 / 190.0).clamp(0.0, 1.0)
        };
        let eased = if self.theme.reduce_motion || !self.theme.ui_animations {
            1.0
        } else {
            Easing::CubicOut.apply(progress)
        };
        let selected = overview.selected;
        let items = &overview.items;

        self.surface.set_opacity(120);
        self.surface.fill_rect(
            Rect::new(0, 0, self.width, self.height),
            Color::rgba(22, 23, 26, 255),
        );
        self.surface.set_opacity(255);

        self.surface.draw_text_centered(
            Rect::new(0, TOP_BAR_HEIGHT + 8, self.width, 30),
            TOP_BAR_HEIGHT + 8,
            "Window Overview",
            HEADLINE,
            Color::rgb(250, 250, 252),
        );

        for (index, item) in items.iter().enumerate() {
            let card = item.card;
            let selected_item = index == selected;
            self.surface.fill_rounded(
                card.inset(-2),
                14,
                if selected_item {
                    self.theme.accent_color().with_alpha(220)
                } else {
                    Color::rgba(0, 0, 0, 110)
                },
            );
            self.surface
                .fill_rounded(card, 12, Color::rgb(250, 250, 252));

            let preview = Rect::new(card.x + 6, card.y + 6, card.w - 12, card.h - 38);
            self.surface.set_clip(preview);
            if !item.scaled.is_empty() && item.scaled_width > 0 {
                self.surface
                    .blit_alpha(preview.x, preview.y, preview.w, preview.h, &item.scaled);
            } else {
                self.surface
                    .fill_rounded(preview, 8, Color::rgb(238, 238, 242));
                ui::draw_app_icon(
                    &mut self.surface,
                    item.kind.icon(),
                    Rect::new(
                        preview.x + preview.w / 2 - 26,
                        preview.y + preview.h / 2 - 26,
                        52,
                        52,
                    ),
                    &self.theme,
                );
            }
            self.surface
                .set_clip(Rect::new(0, 0, self.width, self.height));

            ui::draw_color_tile(
                &mut self.surface,
                Rect::new(card.x + 10, card.bottom() - 26, 16, 16),
                self.theme.accent_color(),
                item.kind.icon(),
                &self.theme,
            );
            let label = format!(
                "{}{}  -  Desktop {}",
                if item.minimized {
                    "[Minimized] "
                } else if item.fullscreen {
                    "[Fullscreen] "
                } else {
                    ""
                },
                item.title,
                item.desktop + 1
            );
            ui::draw_text_truncated(
                &mut self.surface,
                card.x + 32,
                card.bottom() - 24,
                &label,
                CAPTION,
                Color::rgb(40, 40, 44),
                card.w - 44,
            );
        }

        self.surface.draw_text_centered(
            Rect::new(0, self.height - 76, self.width, 24),
            self.height - 76,
            "Arrows select - Enter opens - Esc closes - Click a window",
            CAPTION,
            Color::rgba(255, 255, 255, 190),
        );
    }

    fn open_switcher(&mut self) {
        let now = sys::time_ms();
        let mut items = Vec::new();
        for (id, title, kind, desktop, minimized, fullscreen, normal) in self.wm.overview_items() {
            let mut preview = Vec::new();
            if !minimized {
                let rect = self.wm.window_rect(id, now).unwrap_or(normal);
                if let Some((pixels, width, height)) = self.surface.copy_region(rect) {
                    let thumb_w = 70;
                    let thumb_h = 48;
                    let mut scaled = vec![0u32; (thumb_w * thumb_h) as usize];
                    gfx::upscale_nearest(&pixels, width, height, &mut scaled, thumb_w, thumb_h);
                    preview = scaled;
                }
            }
            items.push(SwitcherItem {
                id,
                title,
                kind,
                desktop,
                minimized,
                fullscreen,
                preview,
            });
        }
        if items.is_empty() {
            return;
        }
        let focus = self.wm.focus;
        let selected = items
            .iter()
            .position(|item| Some(item.id) == focus)
            .map(|index| (index + 1) % items.len())
            .unwrap_or(0);
        self.switcher = Some(Switcher {
            items,
            selected,
            started: now,
        });
        self.needs_full = true;
    }

    fn advance_switcher(&mut self) {
        let Some(switcher) = &mut self.switcher else {
            return;
        };
        if switcher.items.is_empty() {
            return;
        }
        switcher.selected = (switcher.selected + 1) % switcher.items.len();
        self.needs_full = true;
    }

    fn commit_switcher(&mut self) {
        let Some(switcher) = self.switcher.take() else {
            return;
        };
        if let Some(item) = switcher.items.get(switcher.selected) {
            let id = item.id;
            self.wm.focus_window(id);
        }
        self.needs_full = true;
    }

    fn cancel_switcher(&mut self) {
        if self.switcher.take().is_some() {
            self.needs_full = true;
        }
    }

    fn draw_switcher(&mut self, now: u64) {
        let Some(switcher) = &self.switcher else {
            return;
        };
        let count = switcher.items.len().min(7);
        let width = (count as i32 * 92 + 36).min(self.width - 80);
        let height = 170;
        let panel = Rect::new(
            (self.width - width) / 2,
            (self.height - height) / 2 - 10,
            width,
            height,
        );
        let progress = (now.saturating_sub(switcher.started) as f32 / 150.0).clamp(0.0, 1.0);
        let eased = if self.theme.reduce_motion || !self.theme.ui_animations {
            1.0
        } else {
            Easing::CubicOut.apply(progress)
        };
        let previous = self.surface.set_opacity(70);
        self.surface.fill_rect(
            Rect::new(0, 0, self.width, self.height),
            Color::rgba(0, 0, 0, 255),
        );
        self.surface.set_opacity(255);

        let shadow = gfx::shadow_mask(panel.w, panel.h, 18, 22, 22);
        self.surface.rounded_shadow(
            &shadow,
            panel.x - 22,
            panel.y - 18,
            panel.w + 44,
            panel.h + 44,
        );
        self.surface
            .fill_rounded(panel, 18, Color::rgba(250, 250, 252, 255));
        self.surface
            .stroke_rounded(panel, 18, 1, Color::rgba(255, 255, 255, 220));

        let start = switcher.selected.saturating_sub(6);
        for (slot, index) in (start..switcher.items.len()).take(count).enumerate() {
            let item = &switcher.items[index];
            let cell = Rect::new(panel.x + 18 + slot as i32 * 92, panel.y + 16, 80, 92);
            if index == switcher.selected {
                self.surface
                    .fill_rounded(cell, 12, self.theme.accent_color().with_alpha(48));
                self.surface
                    .stroke_rounded(cell, 12, 2, self.theme.accent_color());
            }
            if !item.preview.is_empty() {
                let preview = Rect::new(cell.x + 5, cell.y + 7, 70, 48);
                self.surface
                    .blit_alpha(preview.x, preview.y, preview.w, preview.h, &item.preview);
                self.surface
                    .stroke_rounded(preview, 4, 1, Color::rgba(0, 0, 0, 30));
            } else {
                ui::draw_app_icon(
                    &mut self.surface,
                    item.kind.icon(),
                    Rect::new(cell.x + 16, cell.y + 10, 48, 48),
                    &self.theme,
                );
            }
            ui::draw_text_truncated(
                &mut self.surface,
                cell.x + 6,
                cell.y + 64,
                &item.title,
                CAPTION,
                Color::rgb(50, 50, 55),
                cell.w - 12,
            );
        }

        if let Some(item) = switcher.items.get(switcher.selected) {
            self.surface.draw_text_centered(
                Rect::new(panel.x, panel.bottom() - 44, panel.w, 20),
                panel.bottom() - 44,
                &format!(
                    "{}{}  -  Desktop {}",
                    if item.fullscreen { "[Fullscreen] " } else { "" },
                    item.title,
                    item.desktop + 1
                ),
                BODY_MEDIUM,
                self.theme.palette().text,
            );
        }
        self.surface.draw_text_centered(
            Rect::new(panel.x, panel.bottom() - 24, panel.w, 16),
            panel.bottom() - 24,
            "Alt+Tab cycles - release Alt or press Enter - Esc cancels",
            SMALL,
            self.theme.palette().tertiary,
        );
        self.surface.set_opacity(previous);
    }

    fn begin_workspace_switch(&mut self, delta: i32) {
        let from = self.wm.desktop;
        self.wm.switch_desktop(delta);
        self.start_workspace_animation(from, self.wm.desktop, delta.signum());
    }

    fn begin_workspace_jump(&mut self, index: usize) {
        let from = self.wm.desktop;
        let to = index.min(self.wm.desktop_count.saturating_sub(1));
        if to == from {
            return;
        }
        self.wm.set_desktop(to);
        self.start_workspace_animation(from, to, if to > from { 1 } else { -1 });
    }

    fn start_workspace_animation(&mut self, from: usize, to: usize, direction: i32) {
        self.last_workspace_switch = sys::time_ms();
        let duration = self.theme.workspace_motion(280.0) as u64;
        self.workspace_anim = if duration == 0 || from == to {
            None
        } else {
            Some(WorkspaceAnim {
                from,
                to,
                start: sys::time_ms(),
                duration,
                direction,
            })
        };
        self.needs_full = true;
    }

    fn workspace_progress(&self, now: u64) -> Option<(f32, WorkspaceAnim)> {
        let animation = self.workspace_anim?;
        if now.saturating_sub(animation.start) >= animation.duration {
            return None;
        }
        let t = (now.saturating_sub(animation.start) as f32 / animation.duration as f32)
            .clamp(0.0, 1.0);
        Some((Easing::CubicInOut.apply(t), animation))
    }

    fn copy_wallpaper_offset(&mut self, offset_x: i32) {
        let width = self.width;
        for y in 0..self.height {
            let row = (y * width) as usize;
            for x in 0..width {
                let source = (x - offset_x).rem_euclid(width);
                unsafe {
                    *self.surface.pixels.add(row + x as usize) =
                        *self.wallpaper.as_ptr().add(row + source as usize);
                }
            }
        }
    }

    fn draw_workspace_indicator(&mut self) {
        let now = sys::time_ms();
        if now.saturating_sub(self.last_workspace_switch) > 1100 && self.workspace_anim.is_none() {
            return;
        }
        let dock = self.dock_rect();
        let center = dock.x + dock.w / 2;
        let y = (dock.y - 18).max(TOP_BAR_HEIGHT + 12);
        let total = self.wm.desktop_count as i32;
        let start = center - (total - 1) * 9;
        for index in 0..total {
            let active = index as usize == self.wm.desktop;
            self.surface.fill_circle(
                start + index * 18,
                y,
                if active { 4 } else { 3 },
                if active {
                    self.theme.accent_color()
                } else {
                    Color::rgba(0, 0, 0, 70)
                },
            );
        }
    }

    fn key_press(&mut self, event: &InputEvent) {
        let ctrl = event.mods & mods::CTRL != 0;
        let alt = event.mods & mods::ALT != 0;

        if self.power_prompt.is_some() {
            match event.code {
                aster_abi::keys::ESC => self.close_power_prompt(),
                aster_abi::keys::ENTER => self.confirm_power(),
                _ => {}
            }
            return;
        }

        if alt && event.code == aster_abi::keys::TAB {
            if self.switcher.is_some() {
                self.advance_switcher();
            } else {
                // Alt-Tab outranks a fading overview: replace it immediately
                // instead of swallowing the shortcut until the fade ends.
                self.overview = None;
                self.needs_full = true;
                self.open_switcher();
            }
            return;
        }

        if self.overview.is_some() {
            match event.code {
                aster_abi::keys::ESC => self.close_overview(),
                aster_abi::keys::F3 => self.close_overview(),
                aster_abi::keys::LEFT | aster_abi::keys::UP => self.advance_overview(-1),
                aster_abi::keys::RIGHT | aster_abi::keys::DOWN => self.advance_overview(1),
                aster_abi::keys::ENTER => {
                    if let Some(overview) = &self.overview {
                        if let Some(item) = overview.items.get(overview.selected) {
                            let id = item.id;
                            self.finish_overview(id);
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        if self.switcher.is_some() {
            match event.code {
                aster_abi::keys::TAB => self.advance_switcher(),
                aster_abi::keys::ENTER => self.commit_switcher(),
                aster_abi::keys::ESC => self.cancel_switcher(),
                _ => {}
            }
            return;
        }

        if event.code == aster_abi::keys::ESC && self.wm.fullscreen_active() {
            if let Some(id) = self.wm.focus {
                self.wm.toggle_fullscreen(id);
                self.needs_full = true;
            }
            return;
        }

        if (event.code == aster_abi::keys::F11)
            || ((event.mods & mods::META != 0) && event.code == b'f' as u32)
            || (ctrl && (event.mods & mods::ALT != 0) && event.code == b'f' as u32)
        {
            if let Some(id) = self.wm.focus {
                self.wm.toggle_fullscreen(id);
                self.needs_full = true;
            }
            return;
        }

        if event.code == aster_abi::keys::F3 || (ctrl && event.code == aster_abi::keys::UP) {
            self.open_overview();
            return;
        }

        if self.launcher.is_some() {
            let mut launcher = self.launcher.take().unwrap();

            let mut chosen: Option<AppKind> = None;

            let mut keep = true;

            match event.code {
                aster_abi::keys::ESC => keep = false,

                aster_abi::keys::ENTER => {
                    let results = Self::filter_apps(&launcher.query);

                    chosen = results.get(launcher.selected).copied();
                }

                aster_abi::keys::BACKSPACE => {
                    launcher.query.pop();

                    launcher.selected = 0;
                }

                aster_abi::keys::UP => launcher.selected = launcher.selected.saturating_sub(1),

                aster_abi::keys::DOWN => launcher.selected = launcher.selected.saturating_add(1),

                code if ctrl && code == b' ' as u32 => keep = false,

                code if code >= 32 && code < 127 => {
                    if launcher.query.len() < 48 {
                        launcher.query.push(code as u8 as char);

                        launcher.selected = 0;
                    }
                }

                _ => {}
            }

            if chosen.is_some() {
                keep = false;
            }

            let results = Self::filter_apps(&launcher.query);

            launcher.selected = launcher.selected.min(results.len().saturating_sub(1));

            if keep {
                self.launcher = Some(launcher);
            }

            if let Some(kind) = chosen {
                self.launch(kind);
            }
            return;
        }

        if let Some(menu) = self.open_menu {
            if event.code == aster_abi::keys::ESC {
                self.open_menu = None;

                return;
            }

            let _ = menu;
        }

        if event.code == aster_abi::keys::ESC {
            self.control_center = false;

            self.notif_panel = false;

            self.open_menu = None;

            return;
        }

        if ctrl && event.code == b' ' as u32 {
            self.launcher = Some(Launcher {
                query: String::new(),
                selected: 0,
            });

            return;
        }

        if ctrl && event.code == aster_abi::keys::LEFT {
            self.begin_workspace_switch(-1);
            return;
        }

        if ctrl && event.code == aster_abi::keys::RIGHT {
            self.begin_workspace_switch(1);
            return;
        }

        if ctrl && event.code == aster_abi::keys::TAB {
            self.wm.cycle_focus();

            return;
        }

        if ctrl && (event.code == b'w' as u32 || event.code == b'q' as u32) {
            if let Some(id) = self.wm.focus {
                self.wm.close(id);
            }

            return;
        }

        if ctrl && event.code == b'm' as u32 {
            if let Some(id) = self.wm.focus {
                self.wm.minimize(id);
            }

            return;
        }

        if event.code == aster_abi::keys::F1 {
            self.launcher = Some(Launcher {
                query: String::new(),
                selected: 0,
            });

            return;
        }

        let theme = self.theme;

        let actions = self.wm.key(event, &theme);

        let target = self.wm.focus;

        for action in actions {
            self.apply_action(action, target);
        }
    }

    fn mouse_press(&mut self, x: i32, y: i32) {
        if self.power_prompt.is_some() {
            let (panel, cancel, confirm) = self.power_prompt_layout();
            if cancel.contains(x, y) {
                self.close_power_prompt();
            } else if confirm.contains(x, y) {
                self.confirm_power();
            } else if !panel.contains(x, y) {
                self.close_power_prompt();
            }
            return;
        }
        if self.switcher.is_some() {
            return;
        }
        if self.overview.is_some() {
            let hit = {
                let Some(overview) = &self.overview else {
                    return;
                };
                let layout = self.overview_layout(overview);
                layout
                    .iter()
                    .enumerate()
                    .find(|(_, rect)| rect.contains(x, y))
                    .map(|(index, _)| (index, overview.items[index].id))
            };
            if let Some((index, id)) = hit {
                if let Some(overview) = &mut self.overview {
                    overview.selected = index;
                }
                self.overview_select(id);
            } else {
                self.close_overview();
            }
            return;
        }
        if self.launcher.is_some() {
            let panel = self.launcher_rect();

            if !panel.contains(x, y) {
                self.launcher = None;

                return;
            }

            let launcher = self.launcher.as_ref().unwrap();

            let results = Self::filter_apps(&launcher.query);

            let start = launcher.selected.saturating_sub(5);

            let rows_y = panel.y + 84;

            if y >= rows_y {
                let index = ((y - rows_y) / 42) as usize;

                if let Some(kind) = results.get(start + index).copied() {
                    self.launcher = None;

                    self.launch(kind);
                }
            }

            return;
        }

        if let Some(menu) = self.open_menu {
            let (panel, items) = self.menu_geometry(menu);

            if panel.contains(x, y) {
                let mut cy = panel.y + 7;

                for item in items {
                    if item.separator {
                        cy += 9;

                        continue;
                    }

                    let row = Rect::new(panel.x + 5, cy, panel.w - 10, 26);

                    if row.contains(x, y) {
                        if !item.disabled {
                            if let Some(action) = item.action {
                                self.open_menu = None;

                                self.execute_menu(action);
                            }
                        }

                        return;
                    }

                    cy += 26;
                }

                return;
            }

            self.open_menu = None;
        }

        if self.control_center {
            let panel = self.control_rect();

            if panel.contains(x, y) {
                self.control_click(x, y);
            } else {
                self.control_center = false;
            }

            return;
        }

        if self.notif_panel {
            let panel = self.notif_rect();

            if panel.contains(x, y) {
                if y > panel.bottom() - 42 {
                    self.notifications.clear();
                } else {
                    for notification in &mut self.notifications {
                        notification.read = true;
                    }
                }
            } else {
                self.notif_panel = false;
            }

            return;
        }

        if y < TOP_BAR_HEIGHT {
            self.top_bar_click(x, y);

            return;
        }

        let dock = self.dock_rect();

        if self.dock_hit_rect().contains(x, y) {
            self.dock_click(x, y);

            return;
        }

        let work = self.work_area();

        let theme = self.theme;

        let actions = self.wm.mouse_down(x, y, &theme, work);

        let target = self.wm.focus;

        for action in actions {
            self.apply_action(action, target);
        }
    }

    fn top_bar_click(&mut self, x: i32, y: i32) {
        if !(0..TOP_BAR_HEIGHT).contains(&y) || self.wm.fullscreen_active() {
            return;
        }

        if let Some(kind) = self.menu_at(x, y) {
            self.open_menu = if self.open_menu == Some(kind) {
                None
            } else {
                Some(kind)
            };

            self.control_center = false;

            self.notif_panel = false;

            return;
        }

        let clock_x = self.clock_x();

        let pager_center = self.width / 2;
        let pager_start = pager_center - (self.wm.desktop_count as i32 - 1) * 7;
        for index in 0..self.wm.desktop_count {
            let rect = Rect::new(pager_start + index as i32 * 14 - 8, 5, 16, 16);
            if rect.contains(x, y) {
                self.begin_workspace_jump(index);
                return;
            }
        }

        if x >= clock_x - 8 {
            self.notif_panel = !self.notif_panel;

            self.control_center = false;

            if self.notif_panel {
                for notification in &mut self.notifications {
                    notification.read = true;
                }
            }

            return;
        }

        let wifi_x = clock_x - 26;

        let control_x = clock_x - 56;

        let search_x = clock_x - 84;

        if (x - control_x).abs() < 14 {
            self.control_center = !self.control_center;

            self.notif_panel = false;
        } else if (x - search_x).abs() < 14 {
            self.launcher = Some(Launcher {
                query: String::new(),
                selected: 0,
            });

            self.control_center = false;
        } else if (x - wifi_x).abs() < 14 {
            self.launch(AppKind::Network);
        }
    }

    fn dock_click(&mut self, x: i32, y: i32) {
        for (index, kind) in AppKind::ALL.iter().enumerate() {
            let slot = self.app_slot(index);

            if slot.contains(x, y) {
                self.launch(*kind);

                return;
            }
        }
    }

    fn launch(&mut self, kind: AppKind) {
        let work = self.work_area();

        let theme = self.theme;

        let id = self.wm.open(kind, &theme, work);

        self.wm.focus_id(id);
    }

    fn open_settings_section(&mut self, section: usize) {
        self.launch(AppKind::Settings);

        if let Some(window) = self.wm.focused_mut() {
            if let App::Settings(settings) = &mut window.app {
                settings.set_section(section);
            }
        }
    }

    fn execute_menu(&mut self, action: MenuAction) {
        match action {

            MenuAction::About => self.open_settings_section(10),

            MenuAction::Settings => self.open_settings_section(0),

            MenuAction::CloseWindow => {

                if let Some(id) = self.wm.focus {

                    self.wm.close(id);

                }

            }

            MenuAction::Minimize => {

                if let Some(id) = self.wm.focus {

                    self.wm.minimize(id);

                }

            }

            MenuAction::Zoom => {

                if let Some(id) = self.wm.focus {

                    self.wm.toggle_maximize(id, self.work_area());

                }

            }

            MenuAction::Fullscreen => {
                if let Some(id) = self.wm.focus {
                    self.wm.toggle_fullscreen(id);
                    self.needs_full = true;
                }
            }

            MenuAction::NextWindow => self.wm.cycle_focus(),

            MenuAction::Launcher => {

                self.launcher = Some(Launcher { query: String::new(), selected: 0 })

            }

            MenuAction::Shortcuts => self.notify(

                "Keyboard Shortcuts",

                "Ctrl+Space Spotlight - Ctrl+W Close - Ctrl+M Minimize - Ctrl+Tab Next Window - Ctrl+Left/Right Desktops",

            ),

            MenuAction::ShutDown => self.open_power_prompt(PowerPrompt::Shutdown),

            MenuAction::Restart => self.open_power_prompt(PowerPrompt::Restart),

            MenuAction::Desktop(index) => {

                self.wm.desktop = index.min(self.wm.desktop_count - 1);

                self.wm.focus = self

                    .wm

                    .windows

                    .iter()

                    .filter(|w| w.desktop == self.wm.desktop && !w.minimized && !w.state.is_closed())

                    .max_by_key(|w| w.z)

                    .map(|w| w.id);

            }

            MenuAction::Notifications => {

                self.notif_panel = true;

                for notification in &mut self.notifications {

                    notification.read = true;

                }

            }

        }
    }

    fn apply_action(&mut self, action: Action, target: Option<u64>) {
        match action {
            Action::Close => {
                if let Some(id) = target.or(self.wm.focus) {
                    self.wm.close(id);
                }
            }

            Action::Notify(message) => self.notify("Aster", &message),

            Action::Launch(kind) => self.launch(kind),

            Action::OpenPath(path) => {
                let work = self.work_area();

                let theme = self.theme;

                self.wm.open_path(&path, &theme, work);
            }

            Action::SetAccent(index) => {
                self.theme.accent_index = index.min(theme::ACCENTS.len() - 1);
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SetWallpaper(index) => {
                self.theme.wallpaper = index.min(theme::WALLPAPER_COUNT - 1);

                self.rebuild_wallpaper();

                self.save_preferences();

                self.notify(
                    "Wallpaper",
                    &format!("Applied {}", theme::wallpaper_name(self.theme.wallpaper)),
                );
            }

            Action::SetAnimationSpeed(value) => {
                self.theme.animation_speed = value.clamp(0.25, 2.0);

                self.save_preferences();
            }

            Action::SetReduceMotion(value) => {
                self.theme.reduce_motion = value;

                self.save_preferences();
            }

            Action::SetBrightness(value) => {
                self.theme.brightness = value.clamp(0.3, 1.0);
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SavePreferences => self.save_preferences(),

            Action::RestartServices => {
                self.services.restart();
                self.notify("Services", "The settings service was restarted.");
            }

            Action::SetQuality(quality) => {
                self.theme.quality = quality.min(3);
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SetGraphicsFlag(flag, value) => {
                match flag {
                    0 => self.theme.window_animations = value,
                    1 => self.theme.dock_animations = value,
                    2 => self.theme.workspace_animations = value,
                    3 => self.theme.ui_animations = value,
                    4 => self.theme.smooth_scrolling = value,
                    5 => self.theme.shadows = value,
                    6 => self.theme.transparency = value,
                    7 => self.theme.blur = value,
                    8 => self.theme.dock_indicators = value,
                    9 => self.theme.dock_autohide = value,
                    _ => {}
                }
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SetDockPosition(position) => {
                self.theme.dock_position = position.min(2);
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SetDockSize(size) => {
                self.theme.dock_size = size.clamp(0.7, 1.3);
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SetDockMagnification(value) => {
                self.theme.dock_magnification = value.clamp(0.0, 1.0);
                self.needs_full = true;
                self.save_preferences();
            }

            Action::SetFrameLimit(limit) => {
                self.theme.frame_limit = match limit {
                    0 => 0,
                    30 => 30,
                    120 => 120,
                    _ => 60,
                };
                self.save_preferences();
            }
        }
    }

    fn filter_apps(query: &str) -> Vec<AppKind> {
        let query = query.to_lowercase();

        if query.is_empty() {
            return AppKind::ALL.to_vec();
        }

        AppKind::ALL
            .iter()
            .copied()
            .filter(|kind| kind.title().to_lowercase().contains(&query))
            .collect()
    }

    // -----------------------------------------------------------------

    // Menu bar geometry and menus

    // -----------------------------------------------------------------

    fn menu_titles() -> [(&'static str, MenuKind); 5] {
        [
            ("File", MenuKind::File),
            ("Edit", MenuKind::Edit),
            ("View", MenuKind::View),
            ("Window", MenuKind::Window),
            ("Help", MenuKind::Help),
        ]
    }

    fn app_button(&self) -> Rect {
        let name = self.app_name();

        let width = self.surface.text_width(&name, BODY_MEDIUM) + 14;

        Rect::new(40, 3, width, TOP_BAR_HEIGHT - 6)
    }

    fn menu_at(&self, x: i32, y: i32) -> Option<MenuKind> {
        if !(0..TOP_BAR_HEIGHT).contains(&y) {
            return None;
        }

        if x < 38 || self.app_button().contains(x, y) {
            return Some(MenuKind::App);
        }

        let mut cursor = self.app_button().right() + 10;

        for (title, kind) in Self::menu_titles() {
            let width = self.surface.text_width(title, BODY) + 16;

            let rect = Rect::new(cursor, 3, width, TOP_BAR_HEIGHT - 6);

            if rect.contains(x, y) {
                return Some(kind);
            }

            cursor += width;
        }

        None
    }

    fn menu_button_rect(&self, kind: MenuKind) -> Rect {
        if kind == MenuKind::App {
            return self.app_button();
        }

        let mut cursor = self.app_button().right() + 10;

        for (title, current) in Self::menu_titles() {
            let width = self.surface.text_width(title, BODY) + 16;

            let rect = Rect::new(cursor, 3, width, TOP_BAR_HEIGHT - 6);

            if current == kind {
                return rect;
            }

            cursor += width;
        }

        Rect::new(0, 0, 0, 0)
    }

    fn menu_items(&self, kind: MenuKind) -> Vec<MenuItem> {
        let has_window = self.wm.focus.is_some();

        let maximized = self
            .wm
            .focused()
            .map(|window| window.maximized)
            .unwrap_or(false);

        let fullscreen = self
            .wm
            .focused()
            .map(|window| window.fullscreen)
            .unwrap_or(false);

        match kind {
            MenuKind::App => vec![
                MenuItem::item("About Aster OS", "", MenuAction::About, false),
                MenuItem::item("System Settings...", "", MenuAction::Settings, false),
                MenuItem::separator(),
                MenuItem::item("Restart...", "", MenuAction::Restart, false),
                MenuItem::item("Shut Down...", "", MenuAction::ShutDown, false),
            ],

            MenuKind::File => vec![
                MenuItem::item(
                    "Close Window",
                    "Ctrl W",
                    MenuAction::CloseWindow,
                    !has_window,
                ),
                MenuItem::separator(),
                MenuItem::item(
                    "Open Spotlight...",
                    "Ctrl Space",
                    MenuAction::Launcher,
                    false,
                ),
            ],

            MenuKind::Edit => vec![
                MenuItem::item("Undo", "Ctrl Z", MenuAction::CloseWindow, true),
                MenuItem::item("Redo", "Ctrl Shift Z", MenuAction::CloseWindow, true),
                MenuItem::separator(),
                MenuItem::item("Cut", "Ctrl X", MenuAction::CloseWindow, true),
                MenuItem::item("Copy", "Ctrl C", MenuAction::CloseWindow, true),
                MenuItem::item("Paste", "Ctrl V", MenuAction::CloseWindow, true),
                MenuItem::item("Select All", "Ctrl A", MenuAction::CloseWindow, true),
            ],

            MenuKind::View => vec![
                MenuItem::item("Open Spotlight", "Ctrl Space", MenuAction::Launcher, false),
                MenuItem::item(
                    if maximized {
                        "Exit Full Screen"
                    } else {
                        "Enter Full Screen"
                    },
                    "",
                    MenuAction::Zoom,
                    !has_window,
                ),
                MenuItem::item("Notifications", "", MenuAction::Notifications, false),
            ],

            MenuKind::Window => vec![
                MenuItem::item("Minimize", "Ctrl M", MenuAction::Minimize, !has_window),
                MenuItem::item("Zoom", "", MenuAction::Zoom, !has_window),
                MenuItem::item(
                    if fullscreen {
                        "Exit Fullscreen"
                    } else {
                        "Enter Fullscreen"
                    },
                    "Meta F",
                    MenuAction::Fullscreen,
                    !has_window,
                ),
                MenuItem::item("Next Window", "Ctrl Tab", MenuAction::NextWindow, false),
                MenuItem::separator(),
                MenuItem {
                    label: "Desktop 1",

                    shortcut: "",

                    action: Some(MenuAction::Desktop(0)),

                    disabled: false,

                    checked: self.wm.desktop == 0,

                    separator: false,
                },
                MenuItem {
                    label: "Desktop 2",

                    shortcut: "",

                    action: Some(MenuAction::Desktop(1)),

                    disabled: false,

                    checked: self.wm.desktop == 1,

                    separator: false,
                },
                MenuItem {
                    label: "Desktop 3",

                    shortcut: "",

                    action: Some(MenuAction::Desktop(2)),

                    disabled: false,

                    checked: self.wm.desktop == 2,

                    separator: false,
                },
            ],

            MenuKind::Help => vec![
                MenuItem::item("Keyboard Shortcuts", "", MenuAction::Shortcuts, false),
                MenuItem::item("About Aster OS", "", MenuAction::About, false),
            ],
        }
    }

    fn menu_geometry(&self, kind: MenuKind) -> (Rect, Vec<MenuItem>) {
        let items = self.menu_items(kind);

        let button = self.menu_button_rect(kind);

        let mut height = 14;

        for item in &items {
            height += if item.separator { 9 } else { 26 };
        }

        let x = button.x.min(self.width - MENU_WIDTH - 8).max(8);

        (Rect::new(x, TOP_BAR_HEIGHT + 3, MENU_WIDTH, height), items)
    }

    fn clock_x(&self) -> i32 {
        self.width - 12 - self.surface.text_width(&self.clock_text, BODY)
    }

    // -----------------------------------------------------------------

    // Control Centre

    // -----------------------------------------------------------------

    fn control_click(&mut self, x: i32, y: i32) {
        let panel = self.control_rect();

        let lx = x - panel.x;

        let ly = y - panel.y;

        if ly >= 46 && ly <= 118 {
            if lx >= 14 && lx <= 152 {
                self.launch(AppKind::Network);
            } else if lx >= 160 && lx <= 304 {
                self.notify(
                    "Bluetooth",
                    "No Bluetooth host controller driver is implemented.",
                );
            }

            return;
        }

        if ly >= 126 && ly <= 162 {
            let value = ((lx - 18) as f32 / (panel.w - 36) as f32).clamp(0.0, 1.0);

            self.theme.brightness = value.max(0.3);

            self.dragging_brightness = true;

            self.save_preferences();

            return;
        }

        if ly >= 190 && ly <= 214 {
            self.apply_action(Action::SetReduceMotion(!self.theme.reduce_motion), None);
        }
    }

    // -----------------------------------------------------------------

    // Rendering

    // -----------------------------------------------------------------

    fn effective_quality(&self) -> QualityMode {
        if self.theme.quality == QualityMode::Adaptive.code() {
            if self.stats.average_frame_ms > 26.0 {
                QualityMode::Performance
            } else {
                QualityMode::Balanced
            }
        } else {
            QualityMode::from_code(self.theme.quality)
        }
    }

    fn open_power_prompt(&mut self, prompt: PowerPrompt) {
        self.power_prompt = Some(prompt);
        self.open_menu = None;
        self.control_center = false;
        self.notif_panel = false;
        self.needs_full = true;
        self.dirty.add_all();
    }

    fn close_power_prompt(&mut self) {
        if self.power_prompt.take().is_some() {
            self.needs_full = true;
            self.dirty.add_all();
        }
    }

    fn power_prompt_layout(&self) -> (Rect, Rect, Rect) {
        let panel = Rect::new((self.width - 400) / 2, (self.height - 190) / 2, 400, 190);
        let cancel = Rect::new(panel.x + 24, panel.bottom() - 54, 120, 34);
        let confirm = Rect::new(panel.right() - 174, panel.bottom() - 54, 150, 34);
        (panel, cancel, confirm)
    }

    fn confirm_power(&mut self) {
        match self.power_prompt {
            Some(PowerPrompt::Shutdown) => sys::shutdown(),
            Some(PowerPrompt::Restart) => sys::restart(),
            None => {}
        }
    }

    fn draw_power_prompt(&mut self, prompt: PowerPrompt) {
        let (panel, cancel, confirm) = self.power_prompt_layout();
        self.surface.fill_rect(
            Rect::new(0, 0, self.width, self.height),
            Color::rgba(0, 0, 0, 84),
        );
        self.surface.fill_rounded(
            Rect::new(panel.x + 2, panel.y + 4, panel.w, panel.h),
            18,
            Color::rgba(0, 0, 0, 30),
        );
        self.draw_panel(panel, 18, material::PANEL);
        let (title, body, action) = match prompt {
            PowerPrompt::Shutdown => (
                "Shut Down?",
                "All open apps will close and the machine will power off.",
                "Shut Down",
            ),
            PowerPrompt::Restart => (
                "Restart?",
                "All open apps will close and the machine will restart.",
                "Restart",
            ),
        };
        self.surface.draw_text_centered(
            Rect::new(panel.x, panel.y + 20, panel.w, 26),
            panel.y + 20,
            title,
            HEADLINE,
            self.theme.palette().text,
        );
        self.surface.draw_text_centered(
            Rect::new(panel.x + 20, panel.y + 56, panel.w - 40, 40),
            panel.y + 58,
            body,
            BODY,
            self.theme.palette().secondary,
        );
        self.surface
            .fill_rounded(cancel, 9, Color::rgba(0, 0, 0, 16));
        self.surface.draw_text_centered(
            cancel,
            cancel.y + 9,
            "Cancel",
            BODY_MEDIUM,
            self.theme.palette().text,
        );
        self.surface
            .fill_rounded(confirm, 9, self.theme.accent_color());
        self.surface.draw_text_centered(
            confirm,
            confirm.y + 9,
            action,
            BODY_MEDIUM,
            Color::rgb(255, 255, 255),
        );
    }

    fn draw_panel(&mut self, rect: Rect, radius: i32, material: Material) {
        let quality = self.effective_quality();
        if !self.theme.transparency {
            self.surface
                .fill_rounded(rect, radius, self.theme.palette().surface);
            self.surface
                .stroke_rounded(rect, radius, 1, material.border);
            return;
        }
        if self.theme.blur {
            let quality_ds = match quality {
                QualityMode::Performance => 1,
                QualityMode::Quality => 0,
                _ => 1,
            };
            let quality_radius = match quality {
                QualityMode::Performance => -1,
                QualityMode::Quality => 1,
                _ => 0,
            };
            glass::glass_live(
                &mut self.surface,
                rect,
                radius,
                material.tint,
                (material.blur_radius + quality_radius).max(1),
                (material.downsample + quality_ds).clamp(1, 8),
                material.saturation,
                material.noise,
                &mut self.glass_small,
                &mut self.glass_scratch,
            );
        } else {
            self.surface.fill_rounded(rect, radius, material.tint);
            self.surface
                .stroke_rounded(rect, radius, 1, material.border);
            if radius > 0 {
                self.surface.fill_rect(
                    Rect::new(rect.x + radius, rect.y, rect.w - radius * 2, 1),
                    material.highlight,
                );
            }
        }
    }

    fn draw_top_bar(&mut self) {
        let palette = self.theme.palette();

        let bar = Rect::new(0, 0, self.width, TOP_BAR_HEIGHT);

        self.draw_panel(bar, 0, material::CHROME);

        self.surface.fill_rect(
            Rect::new(0, TOP_BAR_HEIGHT - 1, self.width, 1),
            Color::rgba(0, 0, 0, 22),
        );

        // Brand tile in the menu bar.

        ui::draw_logo_tile(&mut self.surface, Rect::new(10, 2, 22, 22));

        let app_button = self.app_button();

        let app_active = self.open_menu == Some(MenuKind::App)
            || (self.mouse_y < TOP_BAR_HEIGHT && app_button.contains(self.mouse_x, self.mouse_y));

        if app_active {
            self.surface
                .fill_rounded(app_button, 5, Color::rgba(0, 0, 0, 14));
        }

        let name = self.app_name();

        self.surface
            .draw_text(app_button.x + 7, 6, &name, BODY_MEDIUM, palette.text);

        let mut cursor = app_button.right() + 10;

        for (title, kind) in Self::menu_titles() {
            let width = self.surface.text_width(title, BODY) + 16;

            let rect = Rect::new(cursor, 3, width, TOP_BAR_HEIGHT - 6);

            let active = self.open_menu == Some(kind)
                || (self.mouse_y < TOP_BAR_HEIGHT && rect.contains(self.mouse_x, self.mouse_y));

            if active {
                self.surface.fill_rounded(rect, 5, Color::rgba(0, 0, 0, 14));
            }

            self.surface
                .draw_text(rect.x + 8, 6, title, BODY, palette.text);

            cursor += width;
        }

        let clock_x = self.clock_x();

        let pager_center = self.width / 2;
        let pager_start = pager_center - (self.wm.desktop_count as i32 - 1) * 7;
        for index in 0..self.wm.desktop_count {
            let active = index == self.wm.desktop;
            let width = if active { 12 } else { 7 };
            let rect = Rect::new(
                pager_start + index as i32 * 14 - width / 2,
                TOP_BAR_HEIGHT / 2 - 2,
                width,
                4,
            );
            self.surface.fill_rounded(
                rect,
                2,
                if active {
                    self.theme.accent_color()
                } else {
                    Color::rgba(0, 0, 0, 60)
                },
            );
        }

        let control_x = clock_x - 56;

        let search_x = clock_x - 84;

        let wifi_x = clock_x - 26;

        for (x, icon) in [(search_x, Icon::Search), (control_x, Icon::Control)] {
            let rect = Rect::new(x - 8, 5, 16, 16);

            if self.mouse_y < TOP_BAR_HEIGHT && rect.inset(-3).contains(self.mouse_x, self.mouse_y)
            {
                self.surface
                    .fill_rounded(rect.inset(-4), 5, Color::rgba(0, 0, 0, 12));
            }

            ui::draw_icon(&mut self.surface, icon, rect, palette.text, 1, &self.theme);
        }

        let (net_color, net_tooltip) = match self.net {
            Some(status) if status.present == 0 => {
                (palette.tertiary, "No wired adapter".to_string())
            }
            Some(status) if status.configured != 0 => (
                theme::SUCCESS,
                format!("Ethernet {}", network_ip_text(status.ip)),
            ),
            Some(status) if status.link_up != 0 => {
                (theme::WARNING, "Ethernet link up, no address".to_string())
            }
            Some(_) => (palette.tertiary, "Ethernet link down".to_string()),
            None => (palette.tertiary, "No wired adapter".to_string()),
        };

        ui::draw_icon(
            &mut self.surface,
            Icon::Ethernet,
            Rect::new(wifi_x - 8, 5, 16, 16),
            net_color,
            1,
            &self.theme,
        );

        self.surface
            .draw_text(clock_x, 6, &self.clock_text, BODY, palette.text);

        if self.notifications.iter().any(|n| !n.read) {
            self.surface.fill_circle(clock_x - 4, 7, 2, theme::DANGER);
        }

        if self.mouse_y < TOP_BAR_HEIGHT {
            let tooltip = if (self.mouse_x - wifi_x).abs() < 14 {
                Some((net_tooltip.clone(), wifi_x))
            } else if (self.mouse_x - search_x).abs() < 14 {
                Some(("Spotlight".to_string(), search_x))
            } else if (self.mouse_x - control_x).abs() < 14 {
                Some(("Control Centre".to_string(), control_x))
            } else {
                None
            };

            if let Some((text, x)) = tooltip {
                let width = self.surface.text_width(&text, CAPTION) + 16;

                let rect = Rect::new(
                    (x - width / 2).clamp(6, self.width - width - 6),
                    TOP_BAR_HEIGHT + 8,
                    width,
                    22,
                );

                self.surface
                    .fill_rounded(rect, 6, Color::rgba(255, 255, 255, 240));

                self.surface
                    .stroke_rounded(rect, 6, 1, Color::rgba(0, 0, 0, 30));

                self.surface
                    .draw_text(rect.x + 8, rect.y + 4, &text, CAPTION, palette.text);
            }
        }
    }

    fn draw_dock(&mut self) {
        let palette = self.theme.palette();
        let dock = self.dock_rect();
        let shadow_offset = 24;

        if self.theme.shadows {
            if self.dock_shadow_size != (dock.w, dock.h) {
                self.dock_shadow = gfx::shadow_mask(dock.w, dock.h, 20, 24, 26);
                self.dock_shadow_size = (dock.w, dock.h);
            }
            self.surface.rounded_shadow(
                &self.dock_shadow,
                dock.x - shadow_offset,
                dock.y - shadow_offset + 6,
                dock.w + shadow_offset * 2,
                dock.h + shadow_offset * 2,
            );
        }

        self.draw_panel(dock, 20, material::REGULAR);
        self.surface
            .stroke_rounded(dock, 20, 1, Color::rgba(255, 255, 255, 200));

        self.dock_hover = None;
        let slot_size = self.dock_slot_size() as f32;
        let vertical = self.theme.dock_position != 0;

        for (index, kind) in AppKind::ALL.iter().enumerate() {
            let slot = self.app_slot(index);
            let cursor_axis = if vertical { self.mouse_y } else { self.mouse_x };
            let slot_center = if vertical {
                slot.y + slot.h / 2
            } else {
                slot.x + slot.w / 2
            };
            let distance = (cursor_axis - slot_center).abs() as f32;
            let near = if vertical {
                self.mouse_x >= dock.x - 20 && self.mouse_x <= dock.right() + 4
            } else {
                self.mouse_y >= dock.y - 24 && self.mouse_y <= dock.bottom()
            };
            let proximity = if near && self.theme.dock_animations {
                (1.0 - distance / (slot_size * 1.7)).clamp(0.0, 1.0)
            } else if near {
                1.0
            } else {
                0.0
            };
            let hovered = near && distance < slot_size / 2.0;
            if hovered {
                self.dock_hover = Some(index);
            }
            let scale = if self.theme.reduce_motion {
                1.0
            } else {
                1.0 + self.theme.dock_magnification * proximity * proximity
            };
            let icon_rect = self.dock_icon_rect(index, scale);
            ui::draw_app_icon(&mut self.surface, kind.icon(), icon_rect, &self.theme);

            if self.theme.dock_indicators && self.wm.running(*kind) {
                let minimized = self.wm.minimized_count(*kind);
                let (dot_x, dot_y) = if vertical {
                    (slot.right() - 8, slot.y + slot.h / 2)
                } else {
                    (slot.x + slot.w / 2, dock.bottom() - 7)
                };
                self.surface
                    .fill_circle(dot_x, dot_y, 2, Color::rgb(58, 58, 60));
                if minimized > 0 {
                    self.surface
                        .fill_circle(dot_x + 5, dot_y, 1, Color::rgb(120, 120, 124));
                }
            }

            if hovered {
                let label = kind.title();
                let width = self.surface.text_width(label, CAPTION) + 18;
                let (tip_x, tip_y) = if self.theme.dock_position == 1 {
                    (dock.right() + 8, slot.y + slot.h / 2 - 11)
                } else if self.theme.dock_position == 2 {
                    (dock.x - width - 8, slot.y + slot.h / 2 - 11)
                } else {
                    (
                        (slot.x + slot.w / 2 - width / 2).clamp(4, self.width - width - 4),
                        dock.y - 28,
                    )
                };
                let tooltip = Rect::new(tip_x, tip_y, width, 22);
                self.surface
                    .fill_rounded(tooltip, 6, Color::rgba(255, 255, 255, 242));
                self.surface
                    .stroke_rounded(tooltip, 6, 1, Color::rgba(0, 0, 0, 28));
                self.surface
                    .draw_text(tooltip.x + 9, tooltip.y + 4, label, CAPTION, palette.text);
            }
        }
    }

    fn draw_launcher(&mut self) {
        self.surface.fill_rect(
            Rect::new(0, 0, self.width, self.height),
            Color::rgba(0, 0, 0, 36),
        );

        let panel = self.launcher_rect();

        self.surface.fill_rounded(
            Rect::new(panel.x + 2, panel.y + 4, panel.w, panel.h),
            18,
            Color::rgba(0, 0, 0, 26),
        );

        self.draw_panel(panel, 18, material::THICK);

        self.surface
            .stroke_rounded(panel, 18, 1, Color::rgba(255, 255, 255, 190));

        let query = self
            .launcher
            .as_ref()
            .map(|l| l.query.clone())
            .unwrap_or_default();

        let search = Rect::new(panel.x + 18, panel.y + 16, panel.w - 36, 50);

        ui::draw_icon(
            &mut self.surface,
            Icon::Search,
            Rect::new(search.x + 14, search.y + 16, 18, 18),
            self.theme.palette().secondary,
            2,
            &self.theme,
        );

        if query.is_empty() {
            self.surface.draw_text(
                search.x + 42,
                search.y + 13,
                "Spotlight Search",
                HEADLINE,
                self.theme.palette().tertiary,
            );
        } else {
            self.surface.draw_text(
                search.x + 42,
                search.y + 13,
                &query,
                HEADLINE,
                self.theme.palette().text,
            );
        }

        if (sys::time_ms() / 500) % 2 == 0 {
            let caret = search.x + 42 + self.surface.text_width(&query, HEADLINE) + 2;

            self.surface.fill_rect(
                Rect::new(caret, search.y + 13, 2, 25),
                self.theme.accent_color(),
            );
        }

        self.surface.fill_rect(
            Rect::new(panel.x + 18, panel.y + 76, panel.w - 36, 1),
            Color::rgba(0, 0, 0, 18),
        );

        let results = Self::filter_apps(&query);

        let selected = self.launcher.as_ref().map(|l| l.selected).unwrap_or(0);

        let start = selected.saturating_sub(5);

        let rows_y = panel.y + 84;

        if results.is_empty() {
            self.surface.draw_text_centered(
                panel,
                rows_y + 24,
                "No Results",
                BODY,
                self.theme.palette().secondary,
            );

            self.surface.draw_text(
                panel.x + 18,
                panel.bottom() - 22,
                "Enter to open - Esc to close",
                CAPTION,
                self.theme.palette().tertiary,
            );

            return;
        }

        for (slot, kind) in results.iter().enumerate().skip(start).take(6) {
            let row = Rect::new(
                panel.x + 10,
                rows_y + (slot - start) as i32 * 42,
                panel.w - 20,
                38,
            );

            let active = slot == selected;

            if active {
                self.surface.fill_rounded(row, 8, self.theme.accent_color());
            } else if row.contains(self.mouse_x, self.mouse_y) {
                self.surface.fill_rounded(row, 8, Color::rgba(0, 0, 0, 12));
            }

            let icon_rect = Rect::new(row.x + 8, row.y + 4, 30, 30);

            ui::draw_app_icon(&mut self.surface, kind.icon(), icon_rect, &self.theme);

            let text_color = if active {
                Color::rgb(255, 255, 255)
            } else {
                self.theme.palette().text
            };

            let sub_color = if active {
                Color::rgba(255, 255, 255, 200)
            } else {
                self.theme.palette().secondary
            };

            self.surface
                .draw_text(row.x + 50, row.y + 9, kind.title(), BODY_MEDIUM, text_color);

            self.surface.draw_text(
                row.right() - 86,
                row.y + 11,
                "Application",
                CAPTION,
                sub_color,
            );
        }

        self.surface.draw_text(
            panel.x + 18,
            panel.bottom() - 22,
            "Enter to open - Esc to close",
            CAPTION,
            self.theme.palette().tertiary,
        );
    }

    fn draw_control_center(&mut self) {
        let panel = self.control_rect();

        let palette = self.theme.palette();

        self.draw_panel(panel, 18, material::PANEL);

        self.surface.draw_text(
            panel.x + 18,
            panel.y + 14,
            "Control Centre",
            TITLE,
            palette.text,
        );

        // Connectivity tiles reflect the real driver state.

        let ethernet_detail = match self.net {
            Some(status) if status.configured != 0 => network_ip_text(status.ip),
            Some(status) if status.present != 0 && status.link_up != 0 => {
                "Link up, no address".to_string()
            }
            Some(status) if status.present != 0 => "Link down".to_string(),
            _ => "No adapter".to_string(),
        };

        for (index, (icon, title, detail)) in [
            (Icon::Ethernet, "Ethernet", ethernet_detail),
            (Icon::Bluetooth, "Bluetooth", "No adapter".to_string()),
        ]
        .iter()
        .enumerate()
        {
            let tile = Rect::new(panel.x + 14 + index as i32 * 148, panel.y + 46, 144, 70);

            self.surface
                .fill_rounded(tile, 12, Color::rgba(255, 255, 255, 120));

            self.surface
                .fill_circle(tile.x + 26, tile.y + 26, 15, Color::rgb(0xE3, 0xE3, 0xE8));

            ui::draw_icon(
                &mut self.surface,
                *icon,
                Rect::new(tile.x + 18, tile.y + 18, 16, 16),
                palette.secondary,
                1,
                &self.theme,
            );

            self.surface
                .draw_text(tile.x + 48, tile.y + 16, title, BODY_MEDIUM, palette.text);

            self.surface
                .draw_text(tile.x + 48, tile.y + 33, detail, CAPTION, palette.tertiary);
        }

        self.surface.draw_text(
            panel.x + 18,
            panel.y + 128,
            "Display",
            BODY_MEDIUM,
            palette.text,
        );

        self.surface.draw_text(
            panel.x + panel.w - 56,
            panel.y + 128,
            &format!("{}%", (self.theme.brightness * 100.0) as i32),
            CAPTION,
            palette.secondary,
        );

        ui::draw_icon(
            &mut self.surface,
            Icon::Sun,
            Rect::new(panel.x + 18, panel.y + 146, 14, 14),
            palette.secondary,
            1,
            &self.theme,
        );

        ui::draw_slider(
            &mut self.surface,
            Rect::new(panel.x + 40, panel.y + 144, panel.w - 58, 18),
            self.theme.brightness,
            &self.theme,
        );

        self.surface.draw_text(
            panel.x + 18,
            panel.y + 178,
            "Sound",
            BODY_MEDIUM,
            palette.text,
        );

        self.surface.draw_text(
            panel.x + panel.w - 128,
            panel.y + 178,
            "No audio driver",
            CAPTION,
            palette.tertiary,
        );

        let sound_track = Rect::new(panel.x + 18, panel.y + 198, panel.w - 36, 6);

        self.surface
            .fill_rounded(sound_track, 3, Color::rgba(0, 0, 0, 24));

        self.surface.fill_circle(
            panel.x + 18 + 30,
            panel.y + 201,
            7,
            Color::rgb(0xE3, 0xE3, 0xE8),
        );

        self.surface.fill_rect(
            Rect::new(panel.x + 14, panel.y + 220, panel.w - 28, 1),
            Color::rgba(0, 0, 0, 20),
        );

        self.surface.draw_text(
            panel.x + 18,
            panel.y + 234,
            "Reduce motion",
            BODY,
            palette.text,
        );

        ui::draw_toggle(
            &mut self.surface,
            Rect::new(panel.x + panel.w - 58, panel.y + 231, 40, 22),
            self.theme.reduce_motion,
            &self.theme,
        );

        self.surface.draw_text(
            panel.x + 18,
            panel.y + 262,
            "Appearance",
            BODY,
            palette.text,
        );

        self.surface.draw_text(
            panel.x + panel.w - 74,
            panel.y + 262,
            "Light",
            BODY,
            palette.secondary,
        );

        self.surface.draw_text(
            panel.x + 18,
            panel.bottom() - 34,
            "Software renderer - 1280 - 800",
            CAPTION,
            palette.tertiary,
        );

        self.surface.draw_text(
            panel.x + 18,
            panel.bottom() - 19,
            "Aster OS 0.1.0",
            CAPTION,
            palette.tertiary,
        );
    }

    fn draw_menu(&mut self) {
        let Some(kind) = self.open_menu else {
            return;
        };

        let now = sys::time_ms();
        let progress = if self.theme.reduce_motion || !self.theme.ui_animations {
            1.0
        } else {
            (now.saturating_sub(self.menu_started) as f32 / 130.0).clamp(0.0, 1.0)
        };
        let previous_opacity = self
            .surface
            .set_opacity((Easing::CubicOut.apply(progress) * 255.0) as u8);
        let (panel, items) = self.menu_geometry(kind);

        self.surface.fill_rounded(
            Rect::new(panel.x + 2, panel.y + 3, panel.w, panel.h),
            10,
            Color::rgba(0, 0, 0, 22),
        );

        self.surface
            .fill_rounded(panel, 10, Color::rgba(250, 250, 252, 250));

        self.surface
            .stroke_rounded(panel, 10, 1, Color::rgba(0, 0, 0, 30));

        let palette = self.theme.palette();

        let mut cy = panel.y + 7;

        for item in items {
            if item.separator {
                self.surface.fill_rect(
                    Rect::new(panel.x + 12, cy + 4, panel.w - 24, 1),
                    Color::rgba(0, 0, 0, 22),
                );

                cy += 9;

                continue;
            }

            let row = Rect::new(panel.x + 5, cy, panel.w - 10, 26);

            let hovered = row.contains(self.mouse_x, self.mouse_y) && !item.disabled;

            if hovered {
                self.surface.fill_rounded(row, 5, self.theme.accent_color());
            }

            let color = if item.disabled {
                palette.tertiary
            } else if hovered {
                Color::rgb(255, 255, 255)
            } else {
                palette.text
            };

            if item.checked {
                ui::draw_icon(
                    &mut self.surface,
                    Icon::Check,
                    Rect::new(row.x + 6, row.y + 7, 12, 12),
                    color,
                    2,
                    &self.theme,
                );
            }

            self.surface
                .draw_text(row.x + 22, row.y + 7, item.label, BODY, color);

            if !item.shortcut.is_empty() {
                let width = self.surface.text_width(item.shortcut, CAPTION);

                self.surface.draw_text(
                    row.right() - width - 10,
                    row.y + 8,
                    item.shortcut,
                    CAPTION,
                    color,
                );
            }

            cy += 26;
        }
        self.surface.set_opacity(previous_opacity);
    }

    fn draw_menu_fade(&mut self, now: u64) {
        let Some((kind, start)) = self.menu_fade else {
            return;
        };
        let progress = (now.saturating_sub(start) as f32 / 120.0).clamp(0.0, 1.0);
        let saved = self.open_menu;
        self.open_menu = Some(kind);
        let previous = self
            .surface
            .set_opacity(((1.0 - Easing::CubicIn.apply(progress)) * 255.0) as u8);
        self.draw_menu();
        self.surface.set_opacity(previous);
        self.open_menu = saved;
    }

    fn draw_notifications_panel(&mut self) {
        let panel = self.notif_rect();
        let palette = self.theme.palette();
        self.surface.fill_rounded(
            Rect::new(panel.x + 2, panel.y + 4, panel.w, panel.h),
            16,
            Color::rgba(0, 0, 0, 22),
        );
        self.draw_panel(panel, 16, material::PANEL);
        self.surface
            .stroke_rounded(panel, 16, 1, Color::rgba(255, 255, 255, 190));
        self.surface.draw_text(
            panel.x + 16,
            panel.y + 14,
            "Notifications",
            TITLE,
            palette.text,
        );

        if self.notifications.is_empty() {
            self.surface.draw_text_centered(
                panel,
                panel.y + 70,
                "No New Notifications",
                BODY,
                palette.secondary,
            );
            return;
        }

        let mut y = panel.y + 46;
        for notification in self.notifications.iter().rev().take(5) {
            let card = Rect::new(panel.x + 12, y, panel.w - 24, 62);
            self.surface
                .fill_rounded(card, 10, Color::rgb(247, 247, 249));
            self.surface
                .stroke_rounded(card, 10, 1, Color::rgba(0, 0, 0, 14));
            let icon = Rect::new(card.x + 8, card.y + 14, 34, 34);
            ui::draw_logo_tile(&mut self.surface, icon);
            self.surface.draw_text(
                card.x + 50,
                card.y + 8,
                &ui::truncate(&self.surface, &notification.title, BODY_MEDIUM, card.w - 62),
                BODY_MEDIUM,
                palette.text,
            );
            for (line, text) in wrap(&notification.body, 42).iter().take(2).enumerate() {
                self.surface.draw_text(
                    card.x + 50,
                    card.y + 28 + line as i32 * 15,
                    text,
                    CAPTION,
                    palette.secondary,
                );
            }
            y += 68;
        }

        self.surface.fill_rect(
            Rect::new(panel.x + 14, panel.bottom() - 44, panel.w - 28, 1),
            Color::rgba(0, 0, 0, 18),
        );
        let clear = Rect::new(panel.x + 16, panel.bottom() - 34, panel.w - 32, 24);
        if clear.contains(self.mouse_x, self.mouse_y) {
            self.surface
                .fill_rounded(clear, 7, Color::rgba(0, 0, 0, 16));
        }
        self.surface
            .draw_text_centered(clear, clear.y + 4, "Clear All", SMALL, palette.secondary);
    }

    fn draw_toasts(&mut self) {
        let now = sys::time_ms();

        let palette = self.theme.palette();

        let mut index = 0;

        for notification in self.notifications.iter().rev() {
            if notification.read || now.saturating_sub(notification.created) > 6000 {
                continue;
            }

            if index >= 3 {
                break;
            }

            let width = 336;
            let age = now.saturating_sub(notification.created);
            let enter = (age as f32 / 190.0).clamp(0.0, 1.0);
            let exit = if age > 5750 {
                ((age - 5750) as f32 / 250.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let alpha = if self.theme.reduce_motion || !self.theme.ui_animations {
                1.0 - exit
            } else {
                Easing::CubicOut.apply(enter) * (1.0 - Easing::CubicIn.apply(exit))
            };
            if alpha <= 0.01 {
                continue;
            }
            let slide = if self.theme.reduce_motion || !self.theme.ui_animations {
                0
            } else {
                ((1.0 - Easing::CubicOut.apply(enter)) * 46.0) as i32
                    + (Easing::CubicIn.apply(exit) * 24.0) as i32
            };
            let x = self.width - width - 16 + slide;
            let y = TOP_BAR_HEIGHT + 12 + index * 92;
            let card = Rect::new(x, y, width, 82);
            let previous_opacity = self.surface.set_opacity((alpha * 255.0) as u8);

            if self.theme.transparency && self.theme.blur {
                glass::glass_live(
                    &mut self.surface,
                    card,
                    16,
                    material::REGULAR.tint,
                    material::REGULAR.blur_radius,
                    material::REGULAR.downsample,
                    material::REGULAR.saturation,
                    material::REGULAR.noise,
                    &mut self.glass_small,
                    &mut self.glass_scratch,
                );
            } else {
                self.surface.fill_rounded(
                    card,
                    16,
                    if self.theme.transparency {
                        material::REGULAR.tint
                    } else {
                        self.theme.palette().surface
                    },
                );
            }

            self.surface
                .stroke_rounded(card, 16, 1, Color::rgba(255, 255, 255, 200));

            let icon = Rect::new(card.x + 14, card.y + 14, 34, 34);

            ui::draw_logo_tile(&mut self.surface, icon);

            self.surface.draw_text(
                card.x + 58,
                card.y + 12,
                &ui::truncate(&self.surface, &notification.title, BODY_MEDIUM, width - 74),
                BODY_MEDIUM,
                palette.text,
            );

            let body = wrap(&notification.body, 44);

            for (line, text) in body.iter().take(2).enumerate() {
                self.surface.draw_text(
                    card.x + 58,
                    card.y + 32 + line as i32 * 15,
                    text,
                    CAPTION,
                    palette.secondary,
                );
            }

            self.surface.set_opacity(previous_opacity);
            index += 1;
        }
    }

    fn draw_cursor(&mut self) {
        let x = self.mouse_x;

        let y = self.mouse_y;

        let arrow = [
            (x, y),
            (x, y + 17),
            (x + 4, y + 13),
            (x + 7, y + 20),
            (x + 10, y + 19),
            (x + 7, y + 12),
            (x + 12, y + 12),
        ];

        // Soft shadow under the pointer.

        let shadow: Vec<(i32, i32)> = arrow.iter().map(|(px, py)| (px + 1, py + 1)).collect();

        self.surface.fill_polygon(&shadow, Color::rgba(0, 0, 0, 80));

        self.surface.fill_polygon(&arrow, Color::rgb(255, 255, 255));

        for index in 0..arrow.len() {
            let (x0, y0) = arrow[index];

            let (x1, y1) = arrow[(index + 1) % arrow.len()];

            self.surface
                .line(x0, y0, x1, y1, Color::rgba(30, 30, 34, 220));
        }
    }
}

fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();

    let mut current = String::new();

    for ch in text.chars() {
        if current.chars().count() >= max_chars {
            lines.push(current);

            current = String::new();
        }

        current.push(ch);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn network_ip_text(address: [u8; 4]) -> String {
    format!(
        "{}.{}.{}.{}",
        address[0], address[1], address[2], address[3]
    )
}

fn month_short(month: u8) -> &'static str {
    match month {
        1 => "Jan",

        2 => "Feb",

        3 => "Mar",

        4 => "Apr",

        5 => "May",

        6 => "Jun",

        7 => "Jul",

        8 => "Aug",

        9 => "Sep",

        10 => "Oct",

        11 => "Nov",

        _ => "Dec",
    }
}

pub fn run() -> ! {
    sys::log(b"desktop: shell starting");

    let info = match sys::framebuffer_info() {
        Some(info) => info,

        None => {
            sys::log(b"desktop: no framebuffer");

            loop {
                sys::sleep_ms(1000);
            }
        }
    };

    let framebuffer = match sys::framebuffer_map() {
        Some(address) => address as *mut u32,

        None => {
            sys::log(b"desktop: cannot map framebuffer");

            loop {
                sys::sleep_ms(1000);
            }
        }
    };

    let Some(mut desktop) = Desktop::new(
        info.width as i32,
        info.height as i32,
        framebuffer,
        info.stride,
        info.format,
    ) else {
        sys::log(b"desktop: cannot create the compositor surface");

        loop {
            sys::sleep_ms(1000);
        }
    };

    sys::log(b"desktop: DESKTOP-READY");

    desktop.run();
}
