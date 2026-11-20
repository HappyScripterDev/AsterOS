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

use crate::logo_data;

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

struct Launcher {
    query: String,

    selected: usize,
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

enum MenuAction {
    About,

    Settings,

    CloseWindow,

    Minimize,

    Zoom,

    NextWindow,

    Launcher,

    Shortcuts,

    ShutDown,

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

    wm: WindowManager,

    mouse_x: i32,

    mouse_y: i32,

    launcher: Option<Launcher>,

    control_center: bool,

    notif_panel: bool,

    open_menu: Option<MenuKind>,

    notifications: Vec<Notification>,

    dock_hover: Option<usize>,

    dragging_brightness: bool,

    clock_text: String,

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

            wm: WindowManager::new(),

            mouse_x: width / 2,

            mouse_y: height / 2,

            launcher: None,

            control_center: false,

            notif_panel: false,

            open_menu: None,

            notifications: Vec::new(),

            dock_hover: None,

            dragging_brightness: false,

            clock_text: String::new(),

            last_clock: 0,

            started: sys::time_ms(),

            frames: 0,

            frame_accum: 0,
        };

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
        Rect::new(
            6,
            TOP_BAR_HEIGHT + 6,
            self.width - 12,
            self.height - TOP_BAR_HEIGHT - DOCK_HEIGHT - DOCK_GAP - 18,
        )
    }

    fn dock_rect(&self) -> Rect {
        let width = DOCK_SLOT * AppKind::ALL.len() as i32 + 24;

        Rect::new(
            (self.width - width) / 2,
            self.height - DOCK_HEIGHT - DOCK_GAP,
            width,
            DOCK_HEIGHT,
        )
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

        Rect::new(
            dock.x + 12 + index as i32 * DOCK_SLOT,
            dock.y,
            DOCK_SLOT,
            DOCK_HEIGHT,
        )
    }

    fn dock_icon_rect(&self, index: usize, scale: f32) -> Rect {
        let slot = self.app_slot(index);

        let size = (DOCK_ICON as f32 * scale) as i32;

        let cx = slot.x + slot.w / 2;

        let bottom = slot.bottom() - 11;

        Rect::new(cx - size / 2, bottom - size, size, size)
    }

    fn app_name(&self) -> String {
        self.wm
            .focused()
            .map(|window| window.kind().title().to_string())
            .unwrap_or_else(|| "Aster".to_string())
    }

    fn load_preferences(&mut self) {
        let Some(data) = sys::read_file("/home/user/SETTINGS.CNF") else {
            return;
        };

        let text = String::from_utf8_lossy(&data);

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            match key.trim() {
                "accent" => {
                    let names: Vec<String> = theme::ACCENTS
                        .iter()
                        .map(|a| a.name.to_lowercase())
                        .collect();

                    let wanted = value.trim().to_lowercase();

                    if let Some(index) = names.iter().position(|name| name == &wanted) {
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

        self.saved_theme = Some(self.theme);
    }

    fn save_preferences(&mut self) {
        if self.saved_theme == Some(self.theme) {
            return;
        }

        let name = theme::ACCENTS[self.theme.accent_index.min(theme::ACCENTS.len() - 1)]
            .name
            .to_lowercase();

        let content = format!(

            "theme=light\naccent={}\nanimations={:.2}\nwallpaper={}\nreduce_motion={}\nbrightness={:.2}\n",

            name,

            self.theme.animation_speed,

            self.theme.wallpaper,

            if self.theme.reduce_motion { 1 } else { 0 },

            self.theme.brightness

        );

        sys::write_file("/home/user/SETTINGS.CNF", content.as_bytes());

        self.saved_theme = Some(self.theme);
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

            if self.frames <= 3 || self.frames % 120 == 0 {
                let window = if self.frames % 120 == 0 {
                    120.0
                } else {
                    self.frames as f32
                };

                let average = self.frame_accum as f32 / window;

                sys::log(
                    format!(
                        "desktop: frame {} at {} ms, {:.1} ms avg",
                        self.frames,
                        sys::time_ms(),
                        average
                    )
                    .as_bytes(),
                );

                if self.frames % 120 == 0 {
                    self.frame_accum = 0;
                }
            }

            if elapsed < 16 {
                sys::sleep_ms(16 - elapsed);
            }
        }
    }

    // -----------------------------------------------------------------

    // Input

    // -----------------------------------------------------------------

    fn handle_event(&mut self, event: &InputEvent) {
        match event.kind {
            k if k == InputKind::MouseMove as u32 => {
                self.mouse_x = event.x.clamp(0, self.width - 1);

                self.mouse_y = event.y.clamp(0, self.height - 1);

                if self.dragging_brightness {
                    let panel = self.control_rect();

                    let value = ((self.mouse_x - (panel.x + 18)) as f32 / (panel.w - 36) as f32)
                        .clamp(0.0, 1.0);

                    self.theme.brightness = value.max(0.3);
                }

                let work = self.work_area();

                self.wm
                    .mouse_move(self.mouse_x, self.mouse_y, work, TOP_BAR_HEIGHT);

                if self.open_menu.is_some() {
                    if let Some(kind) = self.menu_at(self.mouse_x, self.mouse_y) {
                        self.open_menu = Some(kind);
                    }
                }
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

        if self.launcher.is_none() {
            self.draw_toasts();
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

        if dock.contains(x, y) {
            self.dock_click(x);

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
        if !(0..TOP_BAR_HEIGHT).contains(&y) {
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
            self.notify(
                "Wi-Fi",
                "No wireless adapter driver is implemented in this build.",
            );
        }
    }

    fn dock_click(&mut self, x: i32) {
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

    fn execute_menu(&mut self, action: MenuAction) {
        match action {

            MenuAction::About => self.open_settings_section(9),

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

            MenuAction::NextWindow => self.wm.cycle_focus(),

            MenuAction::Launcher => {

                self.launcher = Some(Launcher { query: String::new(), selected: 0 })

            }

            MenuAction::Shortcuts => self.notify(

                "Keyboard Shortcuts",

                "Ctrl+Space Spotlight - Ctrl+W Close - Ctrl+M Minimize - Ctrl+Tab Next Window - Ctrl+Left/Right Desktops",

            ),

            MenuAction::ShutDown => sys::shutdown(),

            MenuAction::Desktop(index) => {

                self.wm.desktop = index.min(self.wm.desktop_count - 1);

                self.wm.focus = self

                    .wm

                    .windows

                    .iter()

                    .filter(|w| w.desktop == self.wm.desktop && !w.minimized && w.closing_at.is_none())

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

                self.save_preferences();
            }

            Action::SavePreferences => self.save_preferences(),
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

        match kind {
            MenuKind::App => vec![
                MenuItem::item("About Aster OS", "", MenuAction::About, false),
                MenuItem::item("System Settings...", "", MenuAction::Settings, false),
                MenuItem::separator(),
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
                self.notify(
                    "Wi-Fi",
                    "No wireless adapter driver is implemented in this build.",
                );
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

    fn draw(&mut self, now: u64) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.wallpaper.as_ptr(),
                self.surface.pixels,
                (self.width * self.height) as usize,
            );
        }

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
            self.draw_launcher();
        }

        self.draw_dock();

        self.draw_top_bar();

        if self.control_center {
            self.draw_control_center();
        }

        if self.notif_panel {
            self.draw_notifications_panel();
        }

        if self.open_menu.is_some() {
            self.draw_menu();
        }

        self.draw_cursor();

        if self.theme.brightness < 0.999 {
            let alpha = ((1.0 - self.theme.brightness) * 150.0) as u8;

            self.surface.fill_rect(
                Rect::new(0, 0, self.width, self.height),
                Color::rgba(0, 0, 0, alpha),
            );
        }

        self.surface
            .present(self.framebuffer, self.fb_stride, self.fb_format);
    }

    fn draw_top_bar(&mut self) {
        let palette = self.theme.palette();

        let bar = Rect::new(0, 0, self.width, TOP_BAR_HEIGHT);

        self.surface
            .glass(bar, 0, self.theme.palette().menu_bar, &self.blurred, 1.0);

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

        ui::draw_icon(
            &mut self.surface,
            Icon::Wifi,
            Rect::new(wifi_x - 8, 5, 16, 16),
            palette.tertiary,
            1,
            &self.theme,
        );

        self.surface
            .line(wifi_x - 7, 19, wifi_x + 6, 5, palette.tertiary);

        self.surface
            .draw_text(clock_x, 6, &self.clock_text, BODY, palette.text);

        if self.notifications.iter().any(|n| !n.read) {
            self.surface.fill_circle(clock_x - 4, 7, 2, theme::DANGER);
        }

        if self.mouse_y < TOP_BAR_HEIGHT {
            let tooltip = if (self.mouse_x - wifi_x).abs() < 14 {
                Some(("No network adapter", wifi_x))
            } else if (self.mouse_x - search_x).abs() < 14 {
                Some(("Spotlight", search_x))
            } else if (self.mouse_x - control_x).abs() < 14 {
                Some(("Control Centre", control_x))
            } else {
                None
            };

            if let Some((text, x)) = tooltip {
                let width = self.surface.text_width(text, CAPTION) + 16;

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
                    .draw_text(rect.x + 8, rect.y + 4, text, CAPTION, palette.text);
            }
        }
    }

    fn draw_dock(&mut self) {
        let palette = self.theme.palette();

        let dock = self.dock_rect();

        let shadow_offset = 24;

        self.surface.rounded_shadow(
            &self.dock_shadow,
            dock.x - shadow_offset,
            dock.y - shadow_offset + 6,
            dock.w + shadow_offset * 2,
            dock.h + shadow_offset * 2,
        );

        self.surface
            .glass(dock, 20, self.theme.palette().menu_bar, &self.blurred, 1.25);

        self.surface
            .stroke_rounded(dock, 20, 1, Color::rgba(255, 255, 255, 200));

        self.dock_hover = None;

        for (index, kind) in AppKind::ALL.iter().enumerate() {
            let slot = self.app_slot(index);

            let distance = (self.mouse_x - (slot.x + slot.w / 2)).abs() as f32;

            let near = self.mouse_y >= dock.y - 24 && self.mouse_y <= dock.y + dock.h;

            let proximity = if near {
                (1.0 - distance / (DOCK_SLOT as f32 * 1.7)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let hovered = near && distance < DOCK_SLOT as f32 / 2.0;

            if hovered {
                self.dock_hover = Some(index);
            }

            let scale = 1.0 + 0.45 * proximity * proximity;

            let icon_rect = self.dock_icon_rect(index, scale);

            ui::draw_app_icon(&mut self.surface, kind.icon(), icon_rect, &self.theme);

            if self.wm.running(*kind) {
                self.surface.fill_circle(
                    slot.x + slot.w / 2,
                    dock.bottom() - 7,
                    2,
                    Color::rgb(58, 58, 60),
                );
            }

            if hovered {
                let label = kind.title();

                let width = self.surface.text_width(label, CAPTION) + 18;

                let tooltip = Rect::new(
                    (slot.x + slot.w / 2 - width / 2).clamp(4, self.width - width - 4),
                    dock.y - 28,
                    width,
                    22,
                );

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
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.blurred.as_ptr(),
                self.surface.pixels,
                (self.width * self.height) as usize,
            );
        }

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

        self.surface.glass(
            panel,
            18,
            self.theme.palette().menu_bar,
            &self.blurred,
            1.55,
        );

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

        self.surface.glass(
            panel,
            18,
            self.theme.palette().menu_bar,
            &self.blurred,
            1.55,
        );

        self.surface
            .stroke_rounded(panel, 18, 1, Color::rgba(255, 255, 255, 190));

        self.surface.draw_text(
            panel.x + 18,
            panel.y + 14,
            "Control Centre",
            TITLE,
            palette.text,
        );

        // Connectivity tiles (honest: no adapters).

        for (index, (icon, title, detail)) in [
            (Icon::Wifi, "Wi-Fi", "No adapter"),
            (Icon::Bluetooth, "Bluetooth", "No adapter"),
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
    }

    fn draw_notifications_panel(&mut self) {
        let panel = self.notif_rect();
        let palette = self.theme.palette();
        self.surface.fill_rounded(
            Rect::new(panel.x + 2, panel.y + 4, panel.w, panel.h),
            16,
            Color::rgba(0, 0, 0, 22),
        );
        self.surface.glass(
            panel,
            16,
            self.theme.palette().menu_bar,
            &self.blurred,
            1.55,
        );
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

            let x = self.width - width - 16;

            let y = TOP_BAR_HEIGHT + 12 + index * 92;

            let card = Rect::new(x, y, width, 82);

            self.surface
                .glass(card, 16, self.theme.palette().menu_bar, &self.blurred, 1.55);

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
