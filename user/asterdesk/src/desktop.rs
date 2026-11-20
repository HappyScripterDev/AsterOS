//! Aster Desktop shell: top bar, dock, launcher, control centre, notifications
//! and the main compositor loop.
//!
//! The shell runs entirely in ring 3. It owns the framebuffer mapping, polls
//! input events from the kernel, and never touches hardware directly.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use aster_abi::{mods, InputEvent, InputKind};
use crate::apps::{Action, App, AppKind};
use crate::font_data::{BODY, BODY_MEDIUM, CAPTION, HEADLINE, SMALL, TITLE};
use crate::gfx::FloatMath;
use crate::gfx::{self, Color, Rect, Surface};
use crate::sys;
use crate::theme::{self, Theme, DARK, LIGHT, TOP_BAR_HEIGHT};
use crate::ui::{self, Icon};
use crate::wm::{WindowManager, TITLE_HEIGHT};

const DOCK_HEIGHT: i32 = 64;
const DOCK_GAP: i32 = 8;
const DOCK_ICON: i32 = 40;
const DOCK_SLOT: i32 = 58;
const CONTROL_WIDTH: i32 = 320;

struct Notification {
    title: String,
    body: String,
    created: u64,
    read: bool,
}

struct Launcher {
    query: String,
    selected: usize,
}

pub struct Desktop {
    surface: Surface,
    framebuffer: *mut u32,
    fb_stride: u32,
    fb_format: u32,
    width: i32,
    height: i32,
    theme: Theme,
    wallpaper: Vec<u32>,
    blurred: Vec<u32>,
    wm: WindowManager,
    mouse_x: i32,
    mouse_y: i32,
    launcher: Option<Launcher>,
    control_center: bool,
    notif_panel: bool,
    aster_menu: bool,
    notifications: Vec<Notification>,
    dock_hover: Option<usize>,
    dragging_brightness: bool,
    clock_text: String,
    date_text: String,
    last_clock: u64,
    started: u64,
    frames: u64,
    frame_accum: u64,
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
        let mut desktop = Self {
            surface,
            framebuffer,
            fb_stride,
            fb_format,
            width,
            height,
            theme: Theme::default(),
            wallpaper: vec![0; (width * height) as usize],
            blurred: Vec::new(),
            wm: WindowManager::new(),
            mouse_x: width / 2,
            mouse_y: height / 2,
            launcher: None,
            control_center: false,
            notif_panel: false,
            aster_menu: false,
            notifications: Vec::new(),
            dock_hover: None,
            dragging_brightness: false,
            clock_text: "00:00".to_string(),
            date_text: String::new(),
            last_clock: 0,
            started: sys::time_ms(),
            frames: 0,
            frame_accum: 0,
        };
        desktop.load_preferences();
        desktop.rebuild_wallpaper();
        sys::log(b"desktop: preferences loaded");
        desktop.update_clock(true);
        desktop.notify(
            "Welcome to Aster OS",
            "Press Ctrl+Space to open the launcher. The dock has the built-in apps.",
        );
        Some(desktop)
    }

    fn work_area(&self) -> Rect {
        Rect::new(
            8,
            TOP_BAR_HEIGHT + 8,
            self.width - 16,
            self.height - TOP_BAR_HEIGHT - DOCK_HEIGHT - DOCK_GAP - 32,
        )
    }

    fn dock_rect(&self) -> Rect {
        let width = DOCK_SLOT * AppKind::ALL.len() as i32 + 24;
        Rect::new((self.width - width) / 2, self.height - DOCK_HEIGHT - DOCK_GAP, width, DOCK_HEIGHT)
    }

    fn launcher_rect(&self) -> Rect {
        Rect::new((self.width - 720) / 2, 92, 720, 396)
    }

    fn control_rect(&self) -> Rect {
        Rect::new(self.width - CONTROL_WIDTH - 16, TOP_BAR_HEIGHT + 8, CONTROL_WIDTH, 432)
    }

    fn notif_rect(&self) -> Rect {
        Rect::new(self.width - 376, TOP_BAR_HEIGHT + 8, 360, 320)
    }

    fn menu_rect(&self) -> Rect {
        Rect::new(12, TOP_BAR_HEIGHT + 4, 232, 176)
    }

    fn app_slot(&self, index: usize) -> Rect {
        let dock = self.dock_rect();
        Rect::new(dock.x + 12 + index as i32 * DOCK_SLOT, dock.y, DOCK_SLOT, DOCK_HEIGHT)
    }

    fn dock_icon_rect(&self, index: usize, scale: f32) -> Rect {
        let slot = self.app_slot(index);
        let size = (DOCK_ICON as f32 * scale) as i32;
        let cx = slot.x + slot.w / 2;
        let bottom = slot.bottom() - 12;
        Rect::new(cx - size / 2, bottom - size, size, size)
    }

    fn load_preferences(&mut self) {
        let Some(data) = sys::read_file("/home/user/settings.conf") else {
            return;
        };
        let text = String::from_utf8_lossy(&data);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "theme" => self.theme.dark = value.trim() != "light",
                "accent" => {
                    let names = ["blue", "violet", "teal", "rose", "amber", "green"];
                    if let Some(index) = names.iter().position(|name| *name == value.trim()) {
                        self.theme.accent_index = index;
                    }
                }
                "animations" => self.theme.animation_speed = value.trim().parse().unwrap_or(1.0),
                "reduce_motion" => self.theme.reduce_motion = value.trim() == "1",
                "wallpaper" => self.theme.wallpaper = value.trim().parse().unwrap_or(0),
                "brightness" => self.theme.brightness = value.trim().parse().unwrap_or(1.0),
                _ => {}
            }
        }
    }

    fn save_preferences(&self) {
        let names = ["blue", "violet", "teal", "rose", "amber", "green"];
        let content = format!(
            "theme={}\naccent={}\nanimations={:.2}\nwallpaper={}\nreduce_motion={}\nbrightness={:.2}\n",
            if self.theme.dark { "dark" } else { "light" },
            names[self.theme.accent_index.min(names.len() - 1)],
            self.theme.animation_speed,
            self.theme.wallpaper,
            if self.theme.reduce_motion { 1 } else { 0 },
            self.theme.brightness
        );
        sys::write_file("/home/user/settings.conf", content.as_bytes());
    }

    fn rebuild_wallpaper(&mut self) {
        // Paint at half resolution, then upscale. The result is a smooth
        // gradient; full-resolution per-pixel lighting is far too slow for a
        // software compositor under emulation.
        let small_w = (self.width / 2).max(160);
        let small_h = (self.height / 2).max(100);
        let mut small = vec![0u32; (small_w * small_h) as usize];
        theme::paint_wallpaper(
            &mut small,
            small_w,
            small_h,
            self.theme.wallpaper,
            self.theme.dark,
        );
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
        let small_blur = gfx::box_blur(&source, blur_w, blur_h, 3);
        let mut blurred = vec![0u32; (self.width * self.height) as usize];
        gfx::upscale_nearest(&small_blur, blur_w, blur_h, &mut blurred, self.width, self.height);
        self.blurred = blurred;
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

    fn unread(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    fn update_clock(&mut self, force: bool) {
        let now = sys::time_ms();
        if !force && now.saturating_sub(self.last_clock) < 1000 {
            return;
        }
        self.last_clock = now;
        if let Some(time) = sys::rtc() {
            self.clock_text = format!("{:02}:{:02}", time.hour, time.minute);
            let weekday = match time.weekday {
                1 => "Sun",
                2 => "Mon",
                3 => "Tue",
                4 => "Wed",
                5 => "Thu",
                6 => "Fri",
                _ => "Sat",
            };
            self.date_text = format!("{} {:02} {}", weekday, time.day, month_short(time.month));
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            let frame_start = sys::time_ms();
            let mut events: [InputEvent; 64] = unsafe { core::mem::zeroed() };
            let count = sys::poll_input(&mut events);
            for event in &events[..count] {
                self.handle_event(event);
            }
            self.update_clock(false);
            let now = sys::time_ms();
            self.wm.tick(&self.theme);
            self.draw(now);
            let elapsed = sys::time_ms().saturating_sub(frame_start);
            self.frames += 1;
            self.frame_accum += elapsed;
            if self.frames <= 5 || self.frames % 60 == 0 {
                let average = self.frame_accum as f32 / if self.frames % 60 == 0 { 60.0 } else { self.frames as f32 };
                sys::log(
                    format!(
                        "desktop: frame {} at {} ms, {} ms this frame, {:.1} ms avg",
                        self.frames,
                        sys::time_ms(),
                        elapsed,
                        average
                    )
                    .as_bytes(),
                );
                if self.frames % 60 == 0 {
                    self.frame_accum = 0;
                }
            }
            if elapsed < 16 {
                sys::sleep_ms(16 - elapsed);
            }
        }
    }

    // -------------------------------------------------------------------
    // Input
    // -------------------------------------------------------------------

    fn handle_event(&mut self, event: &InputEvent) {
        match event.kind {
            k if k == InputKind::MouseMove as u32 => {
                self.mouse_x = event.x.clamp(0, self.width - 1);
                self.mouse_y = event.y.clamp(0, self.height - 1);
                if self.dragging_brightness {
                    let panel = self.control_rect();
                    let x = self.mouse_x - (panel.x + 16);
                    let value = (x as f32 / (panel.w - 32) as f32).clamp(0.0, 1.0);
                    self.theme.brightness = value.max(0.3);
                }
                let work = self.work_area();
                self.wm.mouse_move(self.mouse_x, self.mouse_y, work, TOP_BAR_HEIGHT);
            }
            k if k == InputKind::MouseButton as u32 => {
                if event.code == 0 {
                    if event.value == 1 {
                        self.mouse_press(self.mouse_x, self.mouse_y);
                    } else {
                        let work = self.work_area();
                        self.wm.mouse_up(self.mouse_x, self.mouse_y, work);
                        self.dragging_brightness = false;
                        self.save_preferences();
                    }
                }
            }
            k if k == InputKind::Key as u32 => {
                if event.value == 1 {
                    self.key_press(event);
                }
            }
            _ => {}
        }
    }

    fn key_press(&mut self, event: &InputEvent) {
        let ctrl = event.mods & mods::CTRL != 0;

        if let Some(mut launcher) = self.launcher.take() {
            let mut chosen: Option<AppKind> = None;
            let mut keep = true;
            match event.code {
                aster_abi::keys::ESC => keep = false,
                aster_abi::keys::ENTER => {
                    let apps = Self::filter_apps(&launcher.query);
                    chosen = apps
                        .get(launcher.selected)
                        .copied()
                        .or_else(|| apps.first().copied());
                }
                aster_abi::keys::BACKSPACE => {
                    launcher.query.pop();
                    launcher.selected = 0;
                }
                aster_abi::keys::LEFT => launcher.selected = launcher.selected.saturating_sub(1),
                aster_abi::keys::RIGHT => launcher.selected = launcher.selected.saturating_add(1),
                aster_abi::keys::UP => launcher.selected = launcher.selected.saturating_sub(5),
                aster_abi::keys::DOWN => launcher.selected = launcher.selected.saturating_add(5),
                code if code >= 32 && code < 127 => {
                    if launcher.query.len() < 40 {
                        launcher.query.push(code as u8 as char);
                        launcher.selected = 0;
                    }
                }
                _ => {}
            }
            if chosen.is_some() {
                keep = false;
            }
            let max = Self::filter_apps(&launcher.query).len().saturating_sub(1);
            launcher.selected = launcher.selected.min(max);
            if keep {
                self.launcher = Some(launcher);
            }
            if let Some(kind) = chosen {
                self.launch(kind);
            }
            return;
        }

        if event.code == aster_abi::keys::ESC {
            self.control_center = false;
            self.notif_panel = false;
            self.aster_menu = false;
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
            self.wm.switch_desktop(-1);
            return;
        }
        if ctrl && event.code == aster_abi::keys::RIGHT {
            self.wm.switch_desktop(1);
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
        // Launcher is modal.
        if self.launcher.is_some() {
            let panel = self.launcher_rect();
            if !panel.contains(x, y) {
                self.launcher = None;
                return;
            }
            let apps = self.filtered_apps();
            let start_x = panel.x + 24;
            let start_y = panel.y + 82;
            for (index, kind) in apps.iter().enumerate() {
                let col = index as i32 % 5;
                let row = index as i32 / 5;
                let cell = Rect::new(start_x + col * 134, start_y + row * 116, 126, 108);
                if cell.contains(x, y) {
                    self.launcher = None;
                    self.launch(*kind);
                    return;
                }
            }
            return;
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
                if y > panel.bottom() - 44 {
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

        if self.aster_menu {
            let panel = self.menu_rect();
            if panel.contains(x, y) {
                let index = ((y - panel.y - 8) / 32).clamp(0, 4);
                match index {
                    0 => {
                        self.aster_menu = false;
                        self.open_settings_section(9);
                    }
                    1 => {
                        self.aster_menu = false;
                        self.open_settings_section(0);
                    }
                    3 => {
                        self.aster_menu = false;
                        self.notify("Lock screen", "Screen locking is not implemented in this build.");
                    }
                    4 => {
                        sys::shutdown();
                    }
                    _ => {}
                }
                return;
            }
            self.aster_menu = false;
        }

        if y < TOP_BAR_HEIGHT {
            self.top_bar_click(x, y);
            return;
        }

        let dock = self.dock_rect();
        if dock.contains(x, y) {
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
        if x >= 16 && x <= 84 {
            self.aster_menu = !self.aster_menu;
            self.control_center = false;
            return;
        }
        if y > TOP_BAR_HEIGHT {
            return;
        }
        let control = self.width - 30;
        let bell = self.width - 62;
        let wifi = self.width - 94;
        let volume = self.width - 124;
        let clock = self.width - 236;
        if (x - control).abs() < 14 {
            self.control_center = !self.control_center;
            self.notif_panel = false;
        } else if (x - bell).abs() < 14 {
            self.notif_panel = !self.notif_panel;
            self.control_center = false;
            if self.notif_panel {
                for notification in &mut self.notifications {
                    notification.read = true;
                }
            }
        } else if (x - wifi).abs() < 14 {
            self.notify("Network", "No network stack is implemented in this build.");
        } else if (x - volume).abs() < 14 {
            self.notify("Audio", "No audio driver is implemented in this build.");
        } else if x >= clock - 10 && x <= self.width - 140 {
            self.launch(AppKind::Calendar);
        }
    }

    fn dock_click(&mut self, x: i32, _y: i32) {
        for (index, kind) in AppKind::ALL.iter().enumerate() {
            let slot = self.app_slot(index);
            if x >= slot.x && x < slot.right() {
                self.launch(*kind);
                return;
            }
        }
    }

    fn launch(&mut self, kind: AppKind) {
        let work = self.work_area();
        let theme = self.theme;
        let id = self.wm.open(kind, &theme, work);
        if let Some(window) = self.wm.windows.iter_mut().find(|w| w.id == id) {
            window.desktop = self.wm.desktop;
            window.minimized = false;
            window.minimize_at = None;
        }
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

    fn control_click(&mut self, x: i32, y: i32) {
        let panel = self.control_rect();
        let local_x = x - panel.x;
        let local_y = y - panel.y;

        if local_y >= 46 && local_y <= 110 {
            if local_x >= 16 && local_x <= 152 {
                self.apply_action(Action::SetTheme(!self.theme.dark), None);
                return;
            }
            if local_x >= 168 && local_x <= 304 {
                self.apply_action(Action::SetReduceMotion(!self.theme.reduce_motion), None);
                return;
            }
        }
        if local_y >= 132 && local_y <= 162 {
            let value = ((local_x - 16) as f32 / (panel.w - 32) as f32).clamp(0.0, 1.0);
            self.theme.brightness = value.max(0.3);
            self.dragging_brightness = true;
            self.save_preferences();
            return;
        }
        if local_y >= 174 && local_y <= 210 {
            let index = ((local_x - 16) / 34).max(0) as usize;
            if index < theme::ACCENTS.len() {
                self.apply_action(Action::SetAccent(index), None);
            }
            return;
        }
        if local_y >= 240 && local_y <= 286 {
            self.notify("Wi-Fi", "No wireless adapter driver is implemented.");
            return;
        }
        if local_y >= 286 && local_y <= 330 {
            self.notify("Bluetooth", "No Bluetooth host controller driver is implemented.");
            return;
        }
        if local_y >= panel.h - 56 && local_y <= panel.h - 18 {
            sys::shutdown();
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
            Action::SetTheme(dark) => {
                self.theme.dark = dark;
                self.rebuild_wallpaper();
                self.save_preferences();
            }
            Action::SetAccent(index) => {
                self.theme.accent_index = index.min(theme::ACCENTS.len() - 1);
                self.save_preferences();
            }
            Action::SetWallpaper(index) => {
                self.theme.wallpaper = index.min(theme::WALLPAPER_COUNT - 1);
                self.rebuild_wallpaper();
                self.save_preferences();
                self.notify("Wallpaper", &format!("Applied {}", theme::wallpaper_name(self.theme.wallpaper)));
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
                self.save_preferences();
            }
            Action::SavePreferences => self.save_preferences(),
        }
    }

    fn filtered_apps(&self) -> Vec<AppKind> {
        let query = self
            .launcher
            .as_ref()
            .map(|launcher| launcher.query.clone())
            .unwrap_or_default();
        Self::filter_apps(&query)
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

    // -------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------

    fn draw(&mut self, now: u64) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.wallpaper.as_ptr(),
                self.surface.pixels,
                (self.width * self.height) as usize,
            );
        }
        let work = self.work_area();
        let dock = self.dock_rect();
        let theme = self.theme;
        self.wm.draw(
            &mut self.surface,
            &theme,
            now,
            (dock.x + dock.w / 2, dock.y + dock.h / 2),
            (self.mouse_x, self.mouse_y),
            TOP_BAR_HEIGHT,
        );

        if self.launcher.is_some() {
            self.draw_launcher(now);
        }

        self.draw_dock();
        self.draw_top_bar();
        if self.control_center {
            self.draw_control_center();
        }
        if self.aster_menu {
            self.draw_menu();
        }
        if self.notif_panel {
            self.draw_notifications_panel();
        }
        if self.launcher.is_none() {
            self.draw_toasts();
        }
        self.draw_cursor();

        if self.theme.brightness < 0.999 {
            let alpha = ((1.0 - self.theme.brightness) * 150.0) as u8;
            self.surface
                .fill_rect(Rect::new(0, 0, self.width, self.height), Color::rgba(0, 0, 0, alpha));
        }

        let _ = work;
        self.surface
            .present(self.framebuffer, self.fb_stride, self.fb_format);
    }

    fn draw_top_bar(&mut self) {
        let palette = self.theme.palette();
        let bar = Rect::new(0, 0, self.width, TOP_BAR_HEIGHT);
        self.surface.glass(bar, 0, palette.glass, &self.blurred, 1.0);
        self.surface.fill_rect(
            Rect::new(0, TOP_BAR_HEIGHT - 1, self.width, 1),
            palette.hairline.with_alpha(90),
        );

        // Star mark.
        let cx = 15;
        let cy = TOP_BAR_HEIGHT / 2;
        for i in 0..8 {
            let angle = i as f32 * core::f32::consts::PI / 4.0;
            let radius = if i % 2 == 0 { 6.0 } else { 3.0 };
            let x = cx + (angle.cos() * radius) as i32;
            let y = cy + (angle.sin() * radius) as i32;
            self.surface.line(cx, cy, x, y, self.theme.accent_color());
        }
        self.surface.fill_circle(cx, cy, 2, self.theme.accent_color());
        let menu_hover = self.mouse_y < TOP_BAR_HEIGHT && self.mouse_x >= 26 && self.mouse_x <= 82;
        if menu_hover || self.aster_menu {
            self.surface.fill_rounded(Rect::new(26, 4, 54, 20), 6, palette.text.with_alpha(20));
        }
        self.surface.draw_text(32, 7, "Aster", BODY_MEDIUM, palette.text);

        let focused = self.wm.focused_title().unwrap_or_else(|| "Desktop".to_string());
        let title = ui::truncate(&self.surface, &focused, BODY, 260);
        self.surface.draw_text(96, 7, &title, BODY, palette.secondary);

        let wifi_x = self.width - 94;
        let volume_x = self.width - 124;
        let bell_x = self.width - 62;
        let control_x = self.width - 30;

        ui::draw_icon(&mut self.surface, Icon::Wifi, Rect::new(wifi_x - 8, 6, 16, 16), palette.tertiary, 1, &self.theme);
        self.surface.line(wifi_x - 7, 20, wifi_x + 6, 5, palette.tertiary);
        ui::draw_icon(&mut self.surface, Icon::Volume, Rect::new(volume_x - 8, 6, 16, 16), palette.tertiary, 1, &self.theme);
        self.surface.line(volume_x - 7, 20, volume_x + 6, 5, palette.tertiary);
        ui::draw_icon(&mut self.surface, Icon::Bell, Rect::new(bell_x - 8, 6, 16, 16), palette.secondary, 1, &self.theme);
        let unread = self.unread();
        if unread > 0 || self.notifications.is_empty() {
            let badge = self.notifications.iter().filter(|n| !n.read && sys::time_ms().saturating_sub(n.created) < 8000).count();
            if badge > 0 {
                self.surface.fill_circle(bell_x + 5, 8, 3, crate::theme::DANGER);
            }
        }
        ui::draw_icon(&mut self.surface, Icon::Control, Rect::new(control_x - 8, 6, 16, 16), palette.secondary, 1, &self.theme);

        let clock_width = self.surface.text_width(&self.clock_text, BODY_MEDIUM);
        let date_width = self.surface.text_width(&self.date_text, CAPTION);
        let clock_x = self.width - 146 - clock_width - date_width;
        self.surface.draw_text(clock_x, 7, &self.date_text, CAPTION, palette.tertiary);
        self.surface.draw_text(clock_x + date_width + 10, 6, &self.clock_text, BODY_MEDIUM, palette.text);

        // Hover tooltips for status items.
        if self.mouse_y <= TOP_BAR_HEIGHT {
            let tooltip = if (self.mouse_x - wifi_x).abs() < 14 {
                Some(("No network stack", wifi_x))
            } else if (self.mouse_x - volume_x).abs() < 14 {
                Some(("No audio driver", volume_x))
            } else {
                None
            };
            if let Some((text, x)) = tooltip {
                let width = self.surface.text_width(text, CAPTION) + 16;
                let rect = Rect::new((x - width / 2).clamp(6, self.width - width - 6), TOP_BAR_HEIGHT + 8, width, 22);
                self.surface.fill_rounded(rect, 7, palette.elevated);
                self.surface.stroke_rounded(rect, 7, 1, palette.hairline);
                self.surface.draw_text(rect.x + 8, rect.y + 4, text, CAPTION, palette.secondary);
            }
        }

        // Desktop indicator (three dots) centred in the free area.
        let base = self.width / 2 - 12;
        for index in 0..self.wm.desktop_count {
            let active = index == self.wm.desktop;
            self.surface.fill_circle(
                base + index as i32 * 12,
                TOP_BAR_HEIGHT / 2,
                if active { 3 } else { 2 },
                if active { self.theme.accent_color() } else { palette.tertiary },
            );
        }
    }

    fn draw_dock(&mut self) {
        let palette = self.theme.palette();
        let dock = self.dock_rect();
        self.surface.glass(dock, 18, palette.glass, &self.blurred, 1.15);
        self.surface.stroke_rounded(dock, 18, 1, palette.hairline.with_alpha(80));
        self.dock_hover = None;

        for (index, kind) in AppKind::ALL.iter().enumerate() {
            let slot = self.app_slot(index);
            let distance = (self.mouse_x - (slot.x + slot.w / 2)).abs() as f32;
            let near = self.mouse_y >= dock.y - 24 && self.mouse_y <= dock.y + dock.h;
            let proximity = if near {
                (1.0 - distance / (DOCK_SLOT as f32 * 1.6)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let hovered = near
                && self.mouse_y <= dock.y + dock.h
                && distance < DOCK_SLOT as f32 / 2.0;
            if hovered {
                self.dock_hover = Some(index);
            }
            let scale = 1.0 + 0.42 * proximity * proximity;
            let icon_rect = self.dock_icon_rect(index, scale);
            ui::draw_app_icon(&mut self.surface, kind.icon(), icon_rect, &self.theme);
            if self.wm.running(*kind) {
                self.surface.fill_circle(
                    slot.x + slot.w / 2,
                    dock.bottom() - 7,
                    2,
                    self.theme.accent_color(),
                );
            }
            if hovered {
                let label = kind.title();
                let width = self.surface.text_width(label, CAPTION) + 18;
                let tooltip = Rect::new(
                    (slot.x + slot.w / 2 - width / 2).clamp(4, self.width - width - 4),
                    dock.y - 30,
                    width,
                    22,
                );
                self.surface.fill_rounded(tooltip, 7, palette.elevated);
                self.surface.stroke_rounded(tooltip, 7, 1, palette.hairline);
                self.surface.draw_text(tooltip.x + 9, tooltip.y + 4, label, CAPTION, palette.text);
            }
        }
    }

    fn draw_launcher(&mut self, _now: u64) {
        // Blurred wallpaper field replaces windows and the dock.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.blurred.as_ptr(),
                self.surface.pixels,
                (self.width * self.height) as usize,
            );
        }
        self.surface.fill_rect(
            Rect::new(0, 0, self.width, self.height),
            Color::rgba(6, 9, 14, 130),
        );
        let panel = self.launcher_rect();
        let palette = self.theme.palette();
        self.surface.glass(panel, 20, palette.glass, &self.blurred, 1.4);
        self.surface.stroke_rounded(panel, 20, 1, palette.hairline.with_alpha(120));

        let search = Rect::new(panel.x + 24, panel.y + 22, panel.w - 48, 42);
        let query = self.launcher.as_ref().map(|l| l.query.clone()).unwrap_or_default();
        self.surface.fill_rounded(search, 9, palette.elevated);
        self.surface.stroke_rounded(search, 9, 1, self.theme.accent_color());
        ui::draw_icon(&mut self.surface, Icon::Search, Rect::new(search.x + 12, search.y + 13, 16, 16), palette.tertiary, 1, &self.theme);
        if query.is_empty() {
            self.surface.draw_text(search.x + 36, search.y + 13, "Search applications", BODY, palette.tertiary);
        } else {
            self.surface.draw_text(search.x + 36, search.y + 13, &query, BODY, palette.text);
            let caret = search.x + 36 + self.surface.text_width(&query, BODY) + 1;
            self.surface.fill_rect(Rect::new(caret, search.y + 12, 1, 16), self.theme.accent_color());
        }

        let apps = self.filtered_apps();
        let selected = self.launcher.as_ref().map(|l| l.selected).unwrap_or(0);
        if apps.is_empty() {
            self.surface.draw_text_centered(panel, panel.y + 150, "No applications match your search", BODY, palette.secondary);
            return;
        }
        let start_x = panel.x + 24;
        let start_y = panel.y + 82;
        for (index, kind) in apps.iter().enumerate() {
            let col = index as i32 % 5;
            let row = index as i32 / 5;
            let cell = Rect::new(start_x + col * 134, start_y + row * 116, 126, 108);
            let hovered = index == selected
                || cell.contains(self.mouse_x, self.mouse_y);
            if hovered {
                self.surface.fill_rounded(cell, 14, palette.text.with_alpha(16));
                self.surface.stroke_rounded(cell, 14, 1, self.theme.accent_color().with_alpha(120));
            }
            let icon = Rect::new(cell.x + (cell.w - 48) / 2, cell.y + 12, 48, 48);
            ui::draw_app_icon(&mut self.surface, kind.icon(), icon, &self.theme);
            self.surface.draw_text_centered(cell, cell.y + 70, kind.title(), SMALL, palette.text);
        }
        self.surface.draw_text(panel.x + 24, panel.bottom() - 26, "Enter to open - Esc to close - arrows to navigate", CAPTION, palette.tertiary);
    }

    fn draw_control_center(&mut self) {
        let panel = self.control_rect();
        let palette = self.theme.palette();
        self.surface.glass(panel, 18, palette.glass, &self.blurred, 1.4);
        self.surface.stroke_rounded(panel, 18, 1, palette.hairline.with_alpha(120));
        self.surface.draw_text(panel.x + 18, panel.y + 16, "Control Centre", TITLE, palette.text);

        // Two large toggles: icon + switch on top, label underneath.
        let tiles = [
            (Icon::Moon, "Dark mode", self.theme.dark),
            (Icon::Accessibility, "Motion", !self.theme.reduce_motion),
        ];
        for (index, (icon, label, value)) in tiles.iter().enumerate() {
            let rect = Rect::new(panel.x + 16 + index as i32 * 152, panel.y + 46, 136, 64);
            self.surface.fill_rounded(rect, 12, palette.elevated);
            ui::draw_icon(&mut self.surface, *icon, Rect::new(rect.x + 10, rect.y + 10, 18, 18), if *value { self.theme.accent_color() } else { palette.secondary }, 1, &self.theme);
            ui::draw_toggle(&mut self.surface, Rect::new(rect.right() - 48, rect.y + 9, 38, 20), *value, &self.theme);
            self.surface.draw_text(rect.x + 10, rect.y + 40, label, SMALL, palette.text);
        }

        self.surface.draw_text(panel.x + 18, panel.y + 126, "Brightness", BODY, palette.text);
        self.surface.draw_text(panel.x + panel.w - 58, panel.y + 126, &format!("{}%", (self.theme.brightness * 100.0) as i32), CAPTION, palette.secondary);
        ui::draw_slider(&mut self.surface, Rect::new(panel.x + 18, panel.y + 144, panel.w - 36, 18), self.theme.brightness, &self.theme);

        self.surface.draw_text(panel.x + 18, panel.y + 174, "Accent", BODY, palette.text);
        for (index, accent) in theme::ACCENTS.iter().enumerate() {
            let cx = panel.x + 26 + index as i32 * 34;
            let cy = panel.y + 200;
            self.surface.fill_circle(cx, cy, 10, accent.color(self.theme.dark));
            if index == self.theme.accent_index {
                self.surface.stroke_circle(cx, cy, 12, 2, palette.text);
            }
        }

        for (index, (icon, title, detail)) in [
            (Icon::Wifi, "Wi-Fi", "No adapter"),
            (Icon::Bluetooth, "Bluetooth", "No adapter"),
        ]
        .iter()
        .enumerate()
        {
            let y = panel.y + 240 + index as i32 * 46;
            self.surface.fill_rounded(Rect::new(panel.x + 16, y, panel.w - 32, 38), 9, palette.elevated);
            ui::draw_icon(&mut self.surface, *icon, Rect::new(panel.x + 28, y + 11, 16, 16), palette.tertiary, 1, &self.theme);
            self.surface.draw_text(panel.x + 52, y + 10, title, BODY, palette.text);
            self.surface.draw_text(panel.x + 140, y + 11, detail, CAPTION, palette.tertiary);
            self.surface.line(panel.x + 28, y + 26, panel.x + 40, y + 14, palette.tertiary);
        }

        self.surface.draw_text(panel.x + 18, panel.y + 334, "Software renderer - 1280x800", CAPTION, palette.tertiary);
        self.surface.draw_text(panel.x + 18, panel.y + 350, "Aster OS 0.1.0", CAPTION, palette.tertiary);
        let shutdown = Rect::new(panel.x + 16, panel.bottom() - 54, panel.w - 32, 36);
        self.surface.fill_rounded(shutdown, 10, crate::theme::DANGER.with_alpha(28));
        self.surface.draw_text_centered(shutdown, shutdown.y + 9, "Shut Down", BODY_MEDIUM, crate::theme::DANGER);
    }

    fn draw_menu(&mut self) {
        let panel = self.menu_rect();
        let palette = self.theme.palette();
        self.surface.glass(panel, 12, palette.glass, &self.blurred, 1.3);
        self.surface.stroke_rounded(panel, 12, 1, palette.hairline);
        let items = [
            ("About Aster OS", false),
            ("System Settings", false),
            ("", true),
            ("Lock Screen", false),
            ("Shut Down", false),
        ];
        let mut y = panel.y + 8;
        for (index, (label, separator)) in items.iter().enumerate() {
            if *separator {
                self.surface.fill_rect(Rect::new(panel.x + 10, y + 4, panel.w - 20, 1), palette.hairline);
                y += 9;
                continue;
            }
            let row = Rect::new(panel.x + 6, y, panel.w - 12, 28);
            if row.contains(self.mouse_x, self.mouse_y) {
                self.surface.fill_rounded(row, 7, self.theme.accent_color().with_alpha(40));
            }
            let color = if index == 4 { crate::theme::DANGER } else { palette.text };
            self.surface.draw_text(row.x + 10, row.y + 7, label, BODY, color);
            if index == 3 {
                self.surface.draw_text(row.right() - 70, row.y + 9, "not yet", CAPTION, palette.tertiary);
            }
            y += 30;
        }
    }

    fn draw_notifications_panel(&mut self) {
        let panel = self.notif_rect();
        let palette = self.theme.palette();
        self.surface.glass(panel, 16, palette.glass, &self.blurred, 1.4);
        self.surface.stroke_rounded(panel, 16, 1, palette.hairline);
        self.surface.draw_text(panel.x + 16, panel.y + 14, "Notifications", TITLE, palette.text);
        let mut y = panel.y + 48;
        if self.notifications.is_empty() {
            self.surface.draw_text(panel.x + 16, y + 10, "No notifications", BODY, palette.secondary);
        }
        for notification in self.notifications.iter().rev().take(6) {
            if y > panel.bottom() - 60 {
                break;
            }
            self.surface.fill_rounded(Rect::new(panel.x + 12, y, panel.w - 24, 52), 10, palette.elevated);
            self.surface.fill_rounded(Rect::new(panel.x + 12, y + 8, 3, 36), 2, self.theme.accent_color());
            self.surface.draw_text(panel.x + 24, y + 8, &ui::truncate(&self.surface, &notification.title, BODY_MEDIUM, panel.w - 60), BODY_MEDIUM, palette.text);
            self.surface.draw_text(panel.x + 24, y + 27, &ui::truncate(&self.surface, &notification.body, CAPTION, panel.w - 60), CAPTION, palette.secondary);
            y += 58;
        }
        let clear = Rect::new(panel.x + 16, panel.bottom() - 40, panel.w - 32, 28);
        if clear.contains(self.mouse_x, self.mouse_y) {
            self.surface.fill_rounded(clear, 8, palette.elevated);
        }
        self.surface.draw_text_centered(clear, clear.y + 6, "Clear all", SMALL, palette.secondary);
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
            let width = 332;
            let x = self.width - width - 18;
            let y = TOP_BAR_HEIGHT + 14 + index * 94;
            let card = Rect::new(x, y, width, 84);
            self.surface.glass(card, 14, palette.glass, &self.blurred, 1.5);
            self.surface.stroke_rounded(card, 14, 1, palette.hairline.with_alpha(120));
            self.surface.fill_rounded(Rect::new(card.x + 12, card.y + 14, 3, 56), 2, self.theme.accent_color());
            self.surface.draw_text(card.x + 24, card.y + 12, &ui::truncate(&self.surface, &notification.title, BODY_MEDIUM, width - 40), BODY_MEDIUM, palette.text);
            let body = wrap(&notification.body, 42);
            for (line, text) in body.iter().take(3).enumerate() {
                self.surface.draw_text(card.x + 24, card.y + 32 + line as i32 * 15, text, CAPTION, palette.secondary);
            }
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
        self.surface.fill_polygon(&arrow, Color::rgb(255, 255, 255));
        for index in 0..arrow.len() {
            let (x0, y0) = arrow[index];
            let (x1, y1) = arrow[(index + 1) % arrow.len()];
            self.surface.line(x0, y0, x1, y1, Color::rgba(10, 12, 18, 200));
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
