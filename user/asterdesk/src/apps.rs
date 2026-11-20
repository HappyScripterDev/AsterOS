//! Built-in Aster applications.
//!
//! Every application draws into the shared surface and uses real kernel
//! services. None of them are mockups: Terminal executes commands against the
//! ramfs, Files lists it, Text and Notes persist files, Monitor reads kernel
//! counters, Calendar reads the CMOS clock.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use aster_abi::{keys, InputEvent, ProcessInfo, SystemInfo};

use crate::font_data::{BODY, BODY_MEDIUM, CAPTION, DISPLAY, HEADLINE, MONO, SMALL, TITLE};
use crate::gfx::FloatMath;
use crate::gfx::{Color, Rect, Surface};
use crate::sys;
use crate::theme::{self, Theme, DANGER, INFO, SUCCESS, WARNING};
use crate::ui::{self, Icon};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppKind {
    Terminal,
    Files,
    Settings,
    Calculator,
    Monitor,
    Text,
    Notes,
    Calendar,
    Photos,
}

impl AppKind {
    pub const ALL: [AppKind; 9] = [
        AppKind::Terminal,
        AppKind::Files,
        AppKind::Settings,
        AppKind::Calculator,
        AppKind::Monitor,
        AppKind::Text,
        AppKind::Notes,
        AppKind::Calendar,
        AppKind::Photos,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            AppKind::Terminal => "Terminal",
            AppKind::Files => "Files",
            AppKind::Settings => "Settings",
            AppKind::Calculator => "Calculator",
            AppKind::Monitor => "Monitor",
            AppKind::Text => "Text",
            AppKind::Notes => "Notes",
            AppKind::Calendar => "Calendar",
            AppKind::Photos => "Wallpapers",
        }
    }

    pub fn icon(&self) -> Icon {
        match self {
            AppKind::Terminal => Icon::Terminal,
            AppKind::Files => Icon::Files,
            AppKind::Settings => Icon::Settings,
            AppKind::Calculator => Icon::Calculator,
            AppKind::Monitor => Icon::Monitor,
            AppKind::Text => Icon::Text,
            AppKind::Notes => Icon::Notes,
            AppKind::Calendar => Icon::Calendar,
            AppKind::Photos => Icon::Photos,
        }
    }
}

#[derive(Clone)]
pub enum Action {
    Close,
    Notify(String),
    OpenPath(String),
    Launch(AppKind),
    SetTheme(bool),
    SetAccent(usize),
    SetWallpaper(usize),
    SetAnimationSpeed(f32),
    SetReduceMotion(bool),
    SetBrightness(f32),
    SavePreferences,
}

pub enum App {
    Terminal(TerminalApp),
    Files(FilesApp),
    Settings(SettingsApp),
    Calculator(CalculatorApp),
    Monitor(MonitorApp),
    Text(TextApp),
    Notes(NotesApp),
    Calendar(CalendarApp),
    Photos(PhotosApp),
}

impl App {
    pub fn new(kind: AppKind, theme: &Theme) -> Self {
        match kind {
            AppKind::Terminal => App::Terminal(TerminalApp::new()),
            AppKind::Files => App::Files(FilesApp::new()),
            AppKind::Settings => App::Settings(SettingsApp::new()),
            AppKind::Calculator => App::Calculator(CalculatorApp::new()),
            AppKind::Monitor => App::Monitor(MonitorApp::new()),
            AppKind::Text => App::Text(TextApp::new(None)),
            AppKind::Notes => App::Notes(NotesApp::new()),
            AppKind::Calendar => App::Calendar(CalendarApp::new()),
            AppKind::Photos => App::Photos(PhotosApp::new(theme)),
        }
    }

    pub fn kind(&self) -> AppKind {
        match self {
            App::Terminal(_) => AppKind::Terminal,
            App::Files(_) => AppKind::Files,
            App::Settings(_) => AppKind::Settings,
            App::Calculator(_) => AppKind::Calculator,
            App::Monitor(_) => AppKind::Monitor,
            App::Text(_) => AppKind::Text,
            App::Notes(_) => AppKind::Notes,
            App::Calendar(_) => AppKind::Calendar,
            App::Photos(_) => AppKind::Photos,
        }
    }

    pub fn title(&self) -> String {
        match self {
            App::Terminal(_) => "Terminal".to_string(),
            App::Files(app) => format!("Files - {}", app.path),
            App::Settings(app) => format!("Settings - {}", app.section_name()),
            App::Calculator(_) => "Calculator".to_string(),
            App::Monitor(_) => "Monitor".to_string(),
            App::Text(app) => format!("Text - {}", app.display_name()),
            App::Notes(_) => "Notes".to_string(),
            App::Calendar(app) => format!("Calendar - {} {}", month_name(app.month), app.year),
            App::Photos(_) => "Wallpapers".to_string(),
        }
    }

    pub fn open_path(&mut self, path: &str) {
        if let App::Text(app) = self {
            app.load(path);
        }
    }

    pub fn tick(&mut self, theme: &Theme) {
        match self {
            App::Monitor(app) => app.tick(),
            App::Calendar(app) => app.tick(),
            App::Photos(app) => app.tick(theme),
            _ => {}
        }
    }

    pub fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        match self {
            App::Terminal(app) => app.draw(surface, content, theme),
            App::Files(app) => app.draw(surface, content, theme),
            App::Settings(app) => app.draw(surface, content, theme),
            App::Calculator(app) => app.draw(surface, content, theme),
            App::Monitor(app) => app.draw(surface, content, theme),
            App::Text(app) => app.draw(surface, content, theme),
            App::Notes(app) => app.draw(surface, content, theme),
            App::Calendar(app) => app.draw(surface, content, theme),
            App::Photos(app) => app.draw(surface, content, theme),
        }
    }

    pub fn key(&mut self, event: &InputEvent, theme: &Theme) -> Option<Action> {
        match self {
            App::Terminal(app) => app.key(event),
            App::Files(app) => app.key(event),
            App::Settings(app) => app.key(event),
            App::Calculator(app) => app.key(event),
            App::Monitor(_) => None,
            App::Text(app) => app.key(event),
            App::Notes(app) => app.key(event),
            App::Calendar(app) => app.key(event),
            App::Photos(app) => app.key(event),
        }
    }

    pub fn mouse(&mut self, x: i32, y: i32, content: Rect, theme: &Theme) -> Option<Action> {
        match self {
            App::Terminal(_) => None,
            App::Files(app) => app.mouse(x, y, content),
            App::Settings(app) => app.mouse(x, y, content, theme),
            App::Calculator(app) => app.mouse(x, y, content, theme),
            App::Monitor(_) => None,
            App::Text(app) => app.mouse(x, y, content),
            App::Notes(app) => app.mouse(x, y, content),
            App::Calendar(app) => app.mouse(x, y, content, theme),
            App::Photos(app) => app.mouse(x, y, content, theme),
        }
    }
}

fn wrap_text(surface: &Surface, text: &str, face: usize, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.split('\n') {
        if raw.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for ch in raw.chars() {
            let mut candidate = current.clone();
            candidate.push(ch);
            if surface.text_width(&candidate, face) > width && !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            current.push(ch);
        }
        lines.push(current);
    }
    lines
}

fn local(content: Rect, x: i32, y: i32) -> (i32, i32) {
    (x - content.x, y - content.y)
}

fn file_icon(name: &str, is_dir: bool) -> Icon {
    if is_dir {
        return Icon::Folder;
    }
    if name.ends_with(".txt") || name.ends_with(".md") || name.ends_with(".conf") || name.ends_with(".log") {
        Icon::Text
    } else if name.ends_with(".png") || name.ends_with(".ppm") {
        Icon::Photos
    } else {
        Icon::File
    }
}

// ===========================================================================
// Terminal
// ===========================================================================

pub struct TerminalApp {
    lines: Vec<String>,
    input: String,
    history: Vec<String>,
    history_index: usize,
    cwd: String,
}

impl TerminalApp {
    fn new() -> Self {
        let mut lines = Vec::new();
        lines.push("Aster Terminal 0.1.0".to_string());
        lines.push("Type `help` for the command list.".to_string());
        if let Some(motd) = sys::read_file("/etc/motd") {
            for line in String::from_utf8_lossy(&motd).lines() {
                lines.push(line.to_string());
            }
        }
        Self {
            lines,
            input: String::new(),
            history: Vec::new(),
            history_index: 0,
            cwd: "/".to_string(),
        }
    }

    fn execute(&mut self) -> Option<Action> {
        let command = self.input.trim().to_string();
        self.input.clear();
        if command.is_empty() {
            return None;
        }
        self.lines.push(format!("aster:{} $ {}", self.cwd, command));
        self.history.push(command.clone());
        self.history_index = self.history.len();
        let mut parts = command.split_whitespace();
        let name = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();
        let mut action = None;
        match name {
            "help" => {
                self.lines.push("Commands:".to_string());
                for line in [
                    "  help                 show this list",
                    "  ls [path]            list a directory",
                    "  cat <path>           print a file",
                    "  write <path> <text>  write a file",
                    "  mkdir <path>         create a directory",
                    "  rm <path>            remove a file or directory",
                    "  pwd / uname / date   system information",
                    "  ps / free            kernel counters",
                    "  theme dark|light     switch the desktop theme",
                    "  accent <name>        change the accent colour",
                    "  open <app>           launch an application",
                    "  echo <text>          print text",
                    "  clear                clear the screen",
                    "  exit                 close the terminal",
                ] {
                    self.lines.push(line.to_string());
                }
            }
            "ls" => {
                let path = args.first().copied().unwrap_or("/");
                match sys::read_dir(path) {
                    Some(entries) => {
                        if entries.is_empty() {
                            self.lines.push("(empty)".to_string());
                        }
                        for (name, is_dir, _) in entries {
                            self.lines.push(format!("{}{}", name, if is_dir { "/" } else { "" }));
                        }
                    }
                    None => self.lines.push(format!("ls: cannot open {}", path)),
                }
            }
            "cat" => {
                if let Some(path) = args.first() {
                    match sys::read_file(path) {
                        Some(data) => {
                            let text = String::from_utf8_lossy(&data).to_string();
                            for line in text.lines().take(200) {
                                self.lines.push(line.to_string());
                            }
                        }
                        None => self.lines.push(format!("cat: {}: no such file", path)),
                    }
                } else {
                    self.lines.push("usage: cat <path>".to_string());
                }
            }
            "write" => {
                if args.len() >= 2 {
                    let path = args[0];
                    let text = args[1..].join(" ");
                    if sys::write_file(path, text.as_bytes()) {
                        self.lines.push(format!("wrote {} bytes to {}", text.len(), path));
                    } else {
                        self.lines.push(format!("write: cannot write {}", path));
                    }
                } else {
                    self.lines.push("usage: write <path> <text>".to_string());
                }
            }
            "mkdir" => {
                if let Some(path) = args.first() {
                    if sys::mkdir(path) {
                        self.lines.push(format!("created {}", path));
                    } else {
                        self.lines.push(format!("mkdir: cannot create {}", path));
                    }
                }
            }
            "rm" => {
                if let Some(path) = args.first() {
                    if sys::unlink(path) {
                        self.lines.push(format!("removed {}", path));
                    } else {
                        self.lines.push(format!("rm: cannot remove {}", path));
                    }
                }
            }
            "pwd" => self.lines.push(self.cwd.clone()),
            "uname" => self.lines.push("Aster OS 0.1.0 x86_64 (aster kernel)".to_string()),
            "date" => match sys::rtc() {
                Some(t) => self.lines.push(format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    t.year, t.month, t.day, t.hour, t.minute, t.second
                )),
                None => self.lines.push("date: RTC unavailable".to_string()),
            },
            "ps" => {
                let mut buffer = [ProcessInfo {
                    pid: 0,
                    threads: 0,
                    state: 0,
                    name: [0; 24],
                }; 16];
                let count = sys::process_list(&mut buffer);
                self.lines.push("  PID  THR  NAME".to_string());
                for process in buffer.iter().take(count) {
                    let name = core::str::from_utf8(&process.name)
                        .unwrap_or("?")
                        .trim_end_matches('\0');
                    self.lines.push(format!("  {:>3}  {:>3}  {}", process.pid, process.threads, name));
                }
            }
            "free" => {
                if let Some(info) = sys::system_info() {
                    self.lines.push(format!(
                        "memory: {} MiB total, {} MiB free, heap {} KiB",
                        info.total_memory_kib / 1024,
                        info.free_memory_kib / 1024,
                        info.kernel_heap_used_kib
                    ));
                    self.lines.push(format!(
                        "threads: {}  processes: {}  switches: {}",
                        info.thread_count, info.process_count, info.context_switches
                    ));
                }
            }
            "theme" => match args.first().copied() {
                Some("dark") => {
                    action = Some(Action::SetTheme(true));
                    self.lines.push("theme: dark".to_string());
                }
                Some("light") => {
                    action = Some(Action::SetTheme(false));
                    self.lines.push("theme: light".to_string());
                }
                _ => self.lines.push("usage: theme dark|light".to_string()),
            },
            "accent" => {
                let names = ["blue", "violet", "teal", "rose", "amber", "green"];
                if let Some(name) = args.first() {
                    if let Some(index) = names.iter().position(|n| n == name) {
                        action = Some(Action::SetAccent(index));
                        self.lines.push(format!("accent: {}", name));
                    } else {
                        self.lines.push(format!("unknown accent '{}'", name));
                    }
                } else {
                    self.lines.push("usage: accent blue|violet|teal|rose|amber|green".to_string());
                }
            }
            "open" => match args.first().copied() {
                Some("files") => action = Some(Action::Launch(AppKind::Files)),
                Some("settings") => action = Some(Action::Launch(AppKind::Settings)),
                Some("monitor") => action = Some(Action::Launch(AppKind::Monitor)),
                Some("notes") => action = Some(Action::Launch(AppKind::Notes)),
                Some("calc") | Some("calculator") => action = Some(Action::Launch(AppKind::Calculator)),
                Some("text") => action = Some(Action::Launch(AppKind::Text)),
                Some("calendar") => action = Some(Action::Launch(AppKind::Calendar)),
                Some("photos") | Some("wallpapers") => action = Some(Action::Launch(AppKind::Photos)),
                _ => self.lines.push("usage: open <files|settings|monitor|notes|calc|text|calendar|photos>".to_string()),
            },
            "echo" => self.lines.push(args.join(" ")),
            "clear" => self.lines.clear(),
            "exit" => action = Some(Action::Close),
            _ => self.lines.push(format!("{}: command not found", name)),
        }
        if self.lines.len() > 500 {
            self.lines.drain(0..100);
        }
        action
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let padding = 12;
        let prompt = format!("aster:{} $ ", self.cwd);
        let prompt_width = surface.text_width(&prompt, MONO);
        let input_line = content.bottom() - padding - 18;
        let text_width = content.w - padding * 2 - prompt_width;

        let mut wrapped: Vec<String> = Vec::new();
        for line in &self.lines {
            if line.is_empty() {
                wrapped.push(String::new());
            } else {
                wrapped.extend(wrap_text(surface, line, MONO, content.w - padding * 2));
            }
        }
        let available = ((input_line - content.y - padding) / 18).max(1) as usize;
        let start = wrapped.len().saturating_sub(available);
        let mut y = content.y + padding;
        for line in &wrapped[start..] {
            surface.draw_text(content.x + padding, y, line, MONO, palette.text);
            y += 18;
        }

        surface.draw_text(content.x + padding, input_line, &prompt, MONO, theme.accent_color());
        let shown = if self.input.is_empty() {
            String::new()
        } else {
            self.input.clone()
        };
        let shown = if surface.text_width(&shown, MONO) > text_width {
            let skip = self.input.len().saturating_sub(48);
            format!("...{}", &self.input[skip.min(self.input.len())..])
        } else {
            shown
        };
        surface.draw_text(content.x + padding + prompt_width, input_line, &shown, MONO, palette.text);
        if (sys::time_ms() / 500) % 2 == 0 {
            let caret = content.x + padding + prompt_width + surface.text_width(&shown, MONO) + 1;
            surface.fill_rect(Rect::new(caret, input_line + 1, 2, 15), theme.accent_color());
        }
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }
        match event.code {
            keys::ENTER => return self.execute(),
            keys::BACKSPACE => {
                self.input.pop();
            }
            keys::UP => {
                if !self.history.is_empty() && self.history_index > 0 {
                    self.history_index -= 1;
                    self.input = self.history[self.history_index].clone();
                }
            }
            keys::DOWN => {
                if self.history_index + 1 < self.history.len() {
                    self.history_index += 1;
                    self.input = self.history[self.history_index].clone();
                } else {
                    self.history_index = self.history.len();
                    self.input.clear();
                }
            }
            keys::ESC => {
                self.input.clear();
            }
            code if code >= 32 && code < 127 => {
                if self.input.len() < 200 {
                    self.input.push(code as u8 as char);
                }
            }
            _ => {}
        }
        None
    }
}

// ===========================================================================
// Files
// ===========================================================================

pub struct FilesApp {
    path: String,
    entries: Vec<(String, bool)>,
    selected: usize,
    scroll: usize,
    back: Vec<String>,
    last_click: (usize, u64),
    status: String,
}

impl FilesApp {
    fn new() -> Self {
        let mut app = Self {
            path: "/home/user".to_string(),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            back: Vec::new(),
            last_click: (usize::MAX, 0),
            status: String::new(),
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        self.entries.clear();
        self.selected = 0;
        self.scroll = 0;
        match sys::read_dir(&self.path) {
            Some(mut entries) => {
                entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                for (name, is_dir, _) in entries {
                    self.entries.push((name, is_dir));
                }
                self.status = format!("{} items", self.entries.len());
            }
            None => {
                self.status = format!("Cannot open {}", self.path);
            }
        }
    }

    fn navigate(&mut self, path: String) {
        if path != self.path {
            self.back.push(self.path.clone());
            self.path = path;
            self.reload();
        }
    }

    fn open_selected(&mut self) -> Option<Action> {
        let (name, is_dir) = self.entries.get(self.selected)?.clone();
        let path = if self.path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", self.path, name)
        };
        if is_dir {
            self.navigate(path);
            None
        } else {
            Some(Action::OpenPath(path))
        }
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let sidebar = Rect::new(content.x, content.y, 176, content.h);
        surface.fill_rect(sidebar, palette.elevated);

        let places = [
            ("Home", "/home/user", Icon::Home),
            ("Documents", "/home/user/Documents", Icon::Folder),
            ("Notes", "/home/user/notes", Icon::Notes),
            ("System", "/system", Icon::Settings),
            ("Logs", "/var/log", Icon::Text),
            ("Root", "/", Icon::Storage),
        ];
        let mut y = content.y + 16;
        for (label, path, icon) in places {
            let row = Rect::new(sidebar.x + 8, y, sidebar.w - 16, 30);
            if self.path == path {
                surface.fill_rounded(row, 8, theme.accent_color().with_alpha(38));
            }
            ui::draw_icon(
                surface,
                icon,
                Rect::new(row.x + 8, row.y + 7, 16, 16),
                if self.path == path { theme.accent_color() } else { palette.secondary },
                1,
                theme,
            );
            surface.draw_text(row.x + 32, row.y + 8, label, BODY, if self.path == path { palette.text } else { palette.secondary });
            y += 32;
        }
        surface.fill_rect(Rect::new(sidebar.right() - 1, sidebar.y, 1, sidebar.h), palette.hairline);

        let toolbar = Rect::new(sidebar.right(), content.y, content.right() - sidebar.right(), 44);
        surface.fill_rect(toolbar, palette.surface);
        surface.fill_rect(Rect::new(toolbar.x, toolbar.bottom() - 1, toolbar.w, 1), palette.hairline);
        let button = |index: i32| Rect::new(toolbar.x + 10 + index * 32, toolbar.y + 10, 26, 24);
        for (index, icon) in [Icon::ChevronLeft, Icon::Up, Icon::Refresh].iter().enumerate() {
            let rect = button(index as i32);
            surface.fill_rounded(rect, 7, palette.elevated);
            ui::draw_icon(surface, *icon, rect.inset(6), palette.secondary, 1, theme);
        }
        let path_color = palette.secondary;
        surface.draw_text(toolbar.x + 116, toolbar.y + 14, &self.path, BODY, path_color);

        let list = Rect::new(toolbar.x, toolbar.bottom(), toolbar.w, content.bottom() - toolbar.bottom());
        let row_height = 32;
        let visible = (list.h / row_height).max(1) as usize;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + visible {
            self.scroll = self.selected - visible + 1;
        }
        let mut y = list.y + 4;
        for (index, (name, is_dir)) in self.entries.iter().enumerate().skip(self.scroll).take(visible) {
            let row = Rect::new(list.x + 8, y, list.w - 16, row_height - 2);
            if index == self.selected {
                surface.fill_rounded(row, 7, theme.accent_color().with_alpha(40));
            }
            let color = if index == self.selected { theme.accent_color() } else { palette.secondary };
            ui::draw_icon(
                surface,
                file_icon(name, *is_dir),
                Rect::new(row.x + 8, row.y + 8, 16, 16),
                color,
                1,
                theme,
            );
            surface.draw_text(row.x + 34, row.y + 8, name, BODY, palette.text);
            if *is_dir {
                surface.draw_text(row.right() - 20, row.y + 8, ">", SMALL, palette.tertiary);
            }
            y += row_height;
        }
        surface.fill_rect(Rect::new(list.x, list.bottom() - 21, list.w, 1), palette.hairline);
        surface.draw_text(list.x + 10, list.bottom() - 17, &self.status, CAPTION, palette.tertiary);
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        if lx < 176 {
            let places = ["/home/user", "/home/user/Documents", "/home/user/notes", "/system", "/var/log", "/"];
            let index = ((ly - 16) / 32).max(0) as usize;
            if ly >= 16 && index < places.len() {
                self.navigate(places[index].to_string());
            }
            return None;
        }
        if ly < 44 {
            if lx >= 10 && lx < 36 {
                if let Some(previous) = self.back.pop() {
                    self.path = previous;
                    self.reload();
                }
            } else if lx >= 42 && lx < 68 {
                if self.path != "/" {
                    let parent = self.path.rsplit_once('/').map(|(p, _)| if p.is_empty() { "/" } else { p }).unwrap_or("/").to_string();
                    self.navigate(parent);
                }
            } else if lx >= 74 && lx < 100 {
                self.reload();
            }
            return None;
        }
        let index = self.scroll + ((ly - 44 - 4) / 32).max(0) as usize;
        if index < self.entries.len() {
            let now = sys::time_ms();
            if self.selected == index && now.saturating_sub(self.last_click.1) < 450 {
                return self.open_selected();
            }
            self.selected = index;
            self.last_click = (index, now);
        }
        None
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }
        match event.code {
            keys::DOWN => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
            }
            keys::UP => {
                self.selected = self.selected.saturating_sub(1);
            }
            keys::ENTER => return self.open_selected(),
            keys::BACKSPACE => {
                if self.path != "/" {
                    let parent = self.path.rsplit_once('/').map(|(p, _)| if p.is_empty() { "/" } else { p }).unwrap_or("/").to_string();
                    self.navigate(parent);
                }
            }
            _ => {}
        }
        None
    }
}

// ===========================================================================
// Settings
// ===========================================================================

pub struct SettingsApp {
    section: usize,
    drag_brightness: bool,
}

const SECTIONS: [(&str, Icon); 10] = [
    ("Appearance", Icon::Palette),
    ("Display", Icon::Display),
    ("Sound", Icon::Sound),
    ("Network", Icon::Network),
    ("Bluetooth", Icon::Bluetooth),
    ("Users", Icon::User),
    ("Security", Icon::Shield),
    ("Storage", Icon::Storage),
    ("Accessibility", Icon::Accessibility),
    ("About", Icon::Info),
];

impl SettingsApp {
    fn new() -> Self {
        Self {
            section: 0,
            drag_brightness: false,
        }
    }

    fn section_name(&self) -> &'static str {
        SECTIONS[self.section].0
    }

    pub fn set_section(&mut self, section: usize) {
        self.section = section.min(SECTIONS.len() - 1);
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let sidebar = Rect::new(content.x, content.y, 176, content.h);
        surface.fill_rect(sidebar, palette.elevated);
        let mut y = content.y + 12;
        for (index, (name, icon)) in SECTIONS.iter().enumerate() {
            let row = Rect::new(sidebar.x + 8, y, sidebar.w - 16, 30);
            if index == self.section {
                surface.fill_rounded(row, 8, theme.accent_color().with_alpha(40));
            }
            ui::draw_icon(
                surface,
                *icon,
                Rect::new(row.x + 8, row.y + 8, 14, 14),
                if index == self.section { theme.accent_color() } else { palette.secondary },
                1,
                theme,
            );
            surface.draw_text(row.x + 30, row.y + 8, name, BODY, palette.text);
            y += 31;
        }
        let body = Rect::new(sidebar.right() + 1, content.y, content.right() - sidebar.right() - 1, content.h);
        let mut cy = body.y + 22;
        let x = body.x + 24;
        surface.draw_text(x, cy, self.section_name(), TITLE, palette.text);
        cy += 32;
        let width = (body.w - 48).max(100);

        match self.section {
            0 => {
                self.row_label(surface, x, cy, width, theme, "Dark mode", "Use the dark appearance");
                cy += 46;
                let toggle = Rect::new(x + width - 44, cy - 46 + 10, 40, 22);
                ui::draw_toggle(surface, toggle, theme.dark, theme);
                surface.draw_text(x, cy, "Accent colour", BODY_MEDIUM, palette.text);
                cy += 26;
                for (index, accent) in theme::ACCENTS.iter().enumerate() {
                    let swatch = Rect::new(x + index as i32 * 34, cy, 26, 26);
                    surface.fill_circle(swatch.x + 13, swatch.y + 13, 11, accent.color(theme.dark));
                    if index == theme.accent_index {
                        surface.stroke_circle(swatch.x + 13, swatch.y + 13, 13, 2, palette.text);
                    }
                }
                cy += 48;
                surface.draw_text(x, cy, "Wallpaper", BODY_MEDIUM, palette.text);
                cy += 24;
                for index in 0..theme::WALLPAPER_COUNT {
                    let tile = Rect::new(x + index as i32 * 92, cy, 82, 46);
                    let (a, b) = wallpaper_swatch(index, theme.dark);
                    surface.vertical_gradient(tile, a, b);
                    surface.stroke_rounded(tile, 8, if index == theme.wallpaper { 2 } else { 1 }, if index == theme.wallpaper { theme.accent_color() } else { palette.hairline });
                    surface.draw_text(tile.x + 6, tile.bottom() - 15, theme::wallpaper_name(index), CAPTION, Color::rgb(255, 255, 255));
                }
                cy += 66;
                surface.draw_text(x, cy, "Animation speed", BODY_MEDIUM, palette.text);
                surface.draw_text(x + width - 40, cy, &format!("{:.2}x", theme.animation_speed), BODY, palette.secondary);
                cy += 24;
                ui::draw_slider(surface, Rect::new(x, cy, width, 20), (theme.animation_speed / 2.0).clamp(0.0, 1.0), theme);
                cy += 36;
                surface.draw_text(x, cy, "Reduce motion", BODY_MEDIUM, palette.text);
                ui::draw_toggle(surface, Rect::new(x + width - 44, cy - 3, 40, 22), theme.reduce_motion, theme);
                cy += 36;
                surface.draw_text(x, cy, "Brightness", BODY_MEDIUM, palette.text);
                cy += 24;
                ui::draw_slider(surface, Rect::new(x, cy, width, 20), theme.brightness.clamp(0.3, 1.0), theme);
            }
            1 => {
                self.info_row(surface, x, &mut cy, width, theme, "Resolution", "1280 x 800");
                self.info_row(surface, x, &mut cy, width, theme, "Colour depth", "32-bit");
                self.info_row(surface, x, &mut cy, width, theme, "Framebuffer", "UEFI GOP, software compositor");
                self.info_row(surface, x, &mut cy, width, theme, "GPU acceleration", "Not implemented (no GPU driver)");
                surface.draw_text(x, cy + 6, "Brightness", BODY_MEDIUM, palette.text);
                ui::draw_slider(surface, Rect::new(x, cy + 30, width, 20), theme.brightness.clamp(0.3, 1.0), theme);
            }
            2 => self.notice_block(surface, x, cy, width, theme, "No audio driver",
                "The kernel does not initialise an HDA or AC'97 controller yet. Volume controls are intentionally absent instead of being decorative."),
            3 => self.notice_block(surface, x, cy, width, theme, "No network stack",
                "Ethernet, IPv4, ARP, DHCP, DNS, TCP and UDP are not implemented. The VirtIO network driver is a future milestone."),
            4 => self.notice_block(surface, x, cy, width, theme, "No Bluetooth adapter",
                "There is no Bluetooth host controller driver. This panel will show discovered devices once the driver exists."),
            5 => {
                self.info_row(surface, x, &mut cy, width, theme, "Account", "user");
                self.info_row(surface, x, &mut cy, width, theme, "Home", "/home/user");
                self.info_row(surface, x, &mut cy, width, theme, "Authentication", "Not implemented");
                surface.draw_text(x, cy + 4, "A single local account exists; there is no password, lock screen or privilege escalation yet.", BODY, palette.secondary);
            }
            6 => {
                let implemented = [
                    ("Ring 3 isolation", "User processes run unprivileged with private page tables."),
                    ("W^X page permissions", "User segments are mapped writable or executable, never both."),
                    ("NX stack and heap", "Data pages are non-executable."),
                    ("Syscall validation", "Every user pointer is translated and range-checked."),
                    ("Panic diagnostics", "Fatal faults print registers instead of silently rebooting."),
                ];
                let missing = [
                    ("Address-space randomisation", "Not implemented."),
                    ("Application sandboxing", "Not implemented."),
                    ("Signed packages and updates", "Not implemented."),
                    ("Audit logging", "Kernel log only."),
                ];
                for (title, detail) in implemented {
                    surface.fill_circle(x + 5, cy + 6, 3, SUCCESS);
                    surface.draw_text(x + 16, cy, title, BODY_MEDIUM, palette.text);
                    surface.draw_text(x + 16, cy + 17, detail, CAPTION, palette.secondary);
                    cy += 40;
                }
                for (title, detail) in missing {
                    surface.fill_circle(x + 5, cy + 6, 3, WARNING);
                    surface.draw_text(x + 16, cy, title, BODY_MEDIUM, palette.text);
                    surface.draw_text(x + 16, cy + 17, detail, CAPTION, palette.secondary);
                    cy += 40;
                }
            }
            7 => {
                self.info_row(surface, x, &mut cy, width, theme, "Filesystem", "ramfs (in memory)");
                self.info_row(surface, x, &mut cy, width, theme, "Persistence", "None: data is lost on shutdown");
                self.info_row(surface, x, &mut cy, width, theme, "On-disk AFS", "Not implemented");
                if let Some(info) = sys::system_info() {
                    self.info_row(surface, x, &mut cy, width, theme, "RAM used", &format!("{} MiB", (info.total_memory_kib - info.free_memory_kib) / 1024));
                    self.info_row(surface, x, &mut cy, width, theme, "RAM free", &format!("{} MiB", info.free_memory_kib / 1024));
                }
            }
            8 => {
                surface.draw_text(x, cy, "Motion", BODY_MEDIUM, palette.text);
                ui::draw_toggle(surface, Rect::new(x + width - 44, cy - 3, 40, 22), theme.reduce_motion, theme);
                cy += 32;
                surface.draw_text(x, cy, "Reduce motion disables window, dock and launcher animations. Contrast follows the selected theme.", BODY, palette.secondary);
                cy += 40;
                surface.draw_text(x, cy, "Interface scale", BODY_MEDIUM, palette.text);
                cy += 22;
                surface.draw_text(x, cy, "100% (scaling is fixed in this build)", BODY, palette.secondary);
            }
            _ => {
                surface.draw_text(x, cy, "Aster OS 0.1.0", HEADLINE, palette.text);
                cy += 36;
                self.info_row(surface, x, &mut cy, width, theme, "Kernel", "aster 0.1.0 (higher half, x86_64)");
                self.info_row(surface, x, &mut cy, width, theme, "ABI", "Aster ABI 1");
                self.info_row(surface, x, &mut cy, width, theme, "Boot", "Aster Boot, UEFI, GPT + FAT32 ESP");
                self.info_row(surface, x, &mut cy, width, theme, "Desktop", "Aster Desktop (ring 3, software renderer)");
                if let Some(info) = sys::system_info() {
                    self.info_row(surface, x, &mut cy, width, theme, "Uptime", &format!("{} s", info.uptime_ms / 1000));
                    self.info_row(surface, x, &mut cy, width, theme, "Processes", &format!("{} processes, {} threads", info.process_count, info.thread_count));
                }
                surface.draw_text(x, cy + 8, "A new generation of personal computing.", BODY, theme.accent_color());
            }
        }
    }

    fn row_label(&self, surface: &mut Surface, x: i32, y: i32, width: i32, theme: &Theme, title: &str, detail: &str) {
        surface.draw_text(x, y, title, BODY_MEDIUM, theme.palette().text);
        surface.draw_text(x, y + 17, detail, CAPTION, theme.palette().secondary);
        let _ = width;
    }

    fn info_row(&self, surface: &mut Surface, x: i32, y: &mut i32, width: i32, theme: &Theme, label: &str, value: &str) {
        let palette = theme.palette();
        surface.draw_text(x, *y, label, BODY, palette.secondary);
        surface.draw_text(x + 190, *y, value, BODY, palette.text);
        surface.fill_rect(Rect::new(x, *y + 24, width, 1), palette.hairline);
        *y += 36;
    }

    fn notice_block(&self, surface: &mut Surface, x: i32, cy: i32, width: i32, theme: &Theme, title: &str, detail: &str) {
        let palette = theme.palette();
        surface.fill_rounded(Rect::new(x, cy, width, 96), 12, palette.elevated);
        ui::draw_icon(surface, Icon::Info, Rect::new(x + 16, cy + 18, 18, 18), WARNING, 2, theme);
        surface.draw_text(x + 44, cy + 18, title, BODY_MEDIUM, palette.text);
        let lines = wrap_text(surface, detail, BODY, width - 60);
        for (index, line) in lines.iter().take(3).enumerate() {
            surface.draw_text(x + 44, cy + 40 + index as i32 * 17, line, BODY, palette.secondary);
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect, theme: &Theme) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        if lx < 176 {
            let index = ((ly - 12) / 31).max(0) as usize;
            if ly >= 12 && index < SECTIONS.len() {
                self.section = index;
            }
            return None;
        }
        let width = (content.w - 176 - 49).max(100);
        let bx = 176 + 25;
        let body_y = ly;
        match self.section {
            0 => {
                if body_y >= 54 && body_y <= 78 {
                    return Some(Action::SetTheme(!theme.dark));
                }
                if body_y >= 125 && body_y <= 154 {
                    let index = ((lx - bx) / 34).max(0) as usize;
                    if index < theme::ACCENTS.len() {
                        return Some(Action::SetAccent(index));
                    }
                }
                if body_y >= 170 && body_y <= 220 {
                    let index = ((lx - bx) / 92).max(0) as usize;
                    if index < theme::WALLPAPER_COUNT {
                        return Some(Action::SetWallpaper(index));
                    }
                }
                if body_y >= 248 && body_y <= 272 {
                    let value = ((lx - bx) as f32 / width as f32).clamp(0.0, 1.0) * 2.0;
                    return Some(Action::SetAnimationSpeed(value.max(0.25)));
                }
                if body_y >= 292 && body_y <= 315 {
                    return Some(Action::SetReduceMotion(!theme.reduce_motion));
                }
                if body_y >= 352 {
                    let value = ((lx - bx) as f32 / width as f32).clamp(0.0, 1.0);
                    self.drag_brightness = true;
                    return Some(Action::SetBrightness(value.max(0.3)));
                }
            }
            1 => {
                if body_y >= 186 {
                    let value = ((lx - bx) as f32 / width as f32).clamp(0.0, 1.0);
                    return Some(Action::SetBrightness(value.max(0.3)));
                }
            }
            _ => {}
        }
        None
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value == 1 {
            if event.code == keys::DOWN {
                self.section = (self.section + 1).min(SECTIONS.len() - 1);
            } else if event.code == keys::UP {
                self.section = self.section.saturating_sub(1);
            }
        }
        None
    }
}

fn wallpaper_swatch(style: usize, dark: bool) -> (Color, Color) {
    match style {
        0 => (
            Color::rgb(if dark { 20 } else { 220 }, if dark { 30 } else { 230 }, if dark { 70 } else { 250 }),
            Color::rgb(if dark { 90 } else { 250 }, if dark { 40 } else { 210 }, if dark { 110 } else { 240 }),
        ),
        1 => (
            Color::rgb(if dark { 10 } else { 220 }, if dark { 45 } else { 240 }, if dark { 55 } else { 245 }),
            Color::rgb(if dark { 30 } else { 180 }, if dark { 110 } else { 240 }, if dark { 95 } else { 225 }),
        ),
        2 => (
            Color::rgb(if dark { 80 } else { 250 }, if dark { 30 } else { 235 }, if dark { 25 } else { 220 }),
            Color::rgb(if dark { 140 } else { 250 }, if dark { 90 } else { 220 }, if dark { 30 } else { 190 }),
        ),
        _ => (
            Color::rgb(if dark { 20 } else { 220 }, if dark { 70 } else { 245 }, if dark { 55 } else { 235 }),
            Color::rgb(if dark { 40 } else { 200 }, if dark { 110 } else { 240 }, if dark { 110 } else { 220 }),
        ),
    }
}

// ===========================================================================
// Calculator
// ===========================================================================

pub struct CalculatorApp {
    expression: String,
    display: String,
    error: bool,
    history: Vec<(String, String)>,
}

impl CalculatorApp {
    fn new() -> Self {
        Self {
            expression: String::new(),
            display: "0".to_string(),
            error: false,
            history: Vec::new(),
        }
    }

    fn buttons() -> [[&'static str; 4]; 5] {
        [
            ["C", "(", ")", "/"],
            ["7", "8", "9", "*"],
            ["4", "5", "6", "-"],
            ["1", "2", "3", "+"],
            ["+/-", "0", ".", "="],
        ]
    }

    fn press(&mut self, key: &str) {
        match key {
            "C" => {
                self.expression.clear();
                self.display = "0".to_string();
                self.error = false;
            }
            "=" => {
                if self.expression.is_empty() {
                    return;
                }
                match evaluate(&self.expression) {
                    Some(value) => {
                        let text = format_number(value);
                        self.history.push((self.expression.clone(), text.clone()));
                        if self.history.len() > 8 {
                            self.history.remove(0);
                        }
                        self.display = text;
                        self.expression.clear();
                        self.error = false;
                    }
                    None => {
                        self.display = "Error".to_string();
                        self.error = true;
                    }
                }
            }
            "+/-" => {
                if self.expression.starts_with('-') {
                    self.expression.remove(0);
                } else {
                    self.expression.insert(0, '-');
                }
            }
            op => {
                self.expression.push_str(op);
                self.display = self.expression.clone();
                self.error = false;
            }
        }
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let display = Rect::new(content.x + 16, content.y + 16, content.w - 32, 92);
        surface.fill_rounded(display, 12, palette.elevated);
        let text = if self.error { "Error" } else { &self.display };
        let shown = ui::truncate(surface, text, DISPLAY, display.w - 24);
        let color = if self.error { DANGER } else { palette.text };
        let width = surface.text_width(&shown, DISPLAY);
        surface.draw_text(display.right() - 16 - width, display.y + 40, &shown, DISPLAY, color);
        if !self.expression.is_empty() {
            let small = ui::truncate(surface, &self.expression, BODY, display.w - 24);
            surface.draw_text(display.x + 14, display.y + 14, &small, BODY, palette.secondary);
        }

        let grid = Rect::new(content.x + 16, display.bottom() + 14, content.w - 32, content.bottom() - display.bottom() - 30);
        let cell_w = grid.w / 4;
        let cell_h = grid.h / 5;
        for (row, buttons) in Self::buttons().iter().enumerate() {
            for (col, label) in buttons.iter().enumerate() {
                let rect = Rect::new(
                    grid.x + col as i32 * cell_w + 3,
                    grid.y + row as i32 * cell_h + 3,
                    cell_w - 6,
                    cell_h - 6,
                );
                let is_operator = ["/", "*", "-", "+", "="].contains(label);
                let background = if *label == "=" {
                    theme.accent_color()
                } else if is_operator {
                    palette.elevated.mix(theme.accent_color(), 0.12)
                } else {
                    palette.elevated
                };
                surface.fill_rounded(rect, 10, background);
                let color = if *label == "=" { theme.text_on_accent() } else { palette.text };
                surface.draw_text_centered(rect, rect.y + (rect.h - 15) / 2, label, TITLE, color);
            }
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect, _theme: &Theme) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        let display_bottom = 16 + 92;
        if ly < display_bottom + 14 {
            return None;
        }
        let grid_y = display_bottom + 14;
        let grid_w = content.w - 32;
        let grid_h = content.h - display_bottom - 30;
        let cell_w = grid_w / 4;
        let cell_h = grid_h / 5;
        let col = ((lx - 16) / cell_w).clamp(0, 3) as usize;
        let row = ((ly - grid_y) / cell_h).clamp(0, 4) as usize;
        self.press(Self::buttons()[row][col]);
        None
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }
        match event.code {
            keys::ENTER => self.press("="),
            keys::BACKSPACE => {
                self.expression.pop();
                self.display = if self.expression.is_empty() { "0".to_string() } else { self.expression.clone() };
            }
            keys::ESC => self.press("C"),
            code if code >= 32 && code < 127 => {
                let ch = code as u8 as char;
                if ch.is_ascii_digit() || "+-*/().".contains(ch) {
                    self.press(&ch.to_string());
                }
            }
            _ => {}
        }
        None
    }
}

fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return "Error".to_string();
    }
    if (value - value.round()).abs() < 1e-9 && value.abs() < 1e15 {
        return format!("{}", value.round() as i64);
    }
    let mut text = format!("{:.6}", value);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn evaluate(input: &str) -> Option<f64> {
    let bytes: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut index = 0usize;
    let value = parse_expr(&bytes, &mut index)?;
    if index != bytes.len() {
        return None;
    }
    Some(value)
}

fn parse_expr(bytes: &[char], index: &mut usize) -> Option<f64> {
    let mut value = parse_term(bytes, index)?;
    while *index < bytes.len() {
        match bytes[*index] {
            '+' => {
                *index += 1;
                value += parse_term(bytes, index)?;
            }
            '-' => {
                *index += 1;
                value -= parse_term(bytes, index)?;
            }
            _ => break,
        }
    }
    Some(value)
}

fn parse_term(bytes: &[char], index: &mut usize) -> Option<f64> {
    let mut value = parse_factor(bytes, index)?;
    while *index < bytes.len() {
        match bytes[*index] {
            '*' => {
                *index += 1;
                value *= parse_factor(bytes, index)?;
            }
            '/' => {
                *index += 1;
                let divisor = parse_factor(bytes, index)?;
                if divisor == 0.0 {
                    return None;
                }
                value /= divisor;
            }
            _ => break,
        }
    }
    Some(value)
}

fn parse_factor(bytes: &[char], index: &mut usize) -> Option<f64> {
    if *index >= bytes.len() {
        return None;
    }
    if bytes[*index] == '-' {
        *index += 1;
        return Some(-parse_factor(bytes, index)?);
    }
    if bytes[*index] == '+' {
        *index += 1;
        return parse_factor(bytes, index);
    }
    if bytes[*index] == '(' {
        *index += 1;
        let value = parse_expr(bytes, index)?;
        if *index >= bytes.len() || bytes[*index] != ')' {
            return None;
        }
        *index += 1;
        return Some(value);
    }
    let start = *index;
    let mut seen_dot = false;
    while *index < bytes.len() {
        let c = bytes[*index];
        if c.is_ascii_digit() {
            *index += 1;
        } else if c == '.' && !seen_dot {
            seen_dot = true;
            *index += 1;
        } else {
            break;
        }
    }
    if start == *index {
        return None;
    }
    let text: String = bytes[start..*index].iter().collect();
    text.parse::<f64>().ok()
}

// ===========================================================================
// Monitor
// ===========================================================================

pub struct MonitorApp {
    info: Option<SystemInfo>,
    processes: Vec<ProcessInfo>,
    memory: Vec<f32>,
    activity: Vec<f32>,
    last_update: u64,
    last_switches: u64,
    last_ticks: u64,
}

impl MonitorApp {
    fn new() -> Self {
        let mut app = Self {
            info: None,
            processes: Vec::new(),
            memory: vec![0.0; 80],
            activity: vec![0.0; 80],
            last_update: 0,
            last_switches: 0,
            last_ticks: 0,
        };
        app.tick();
        app
    }

    fn tick(&mut self) {
        let now = sys::time_ms();
        if now.saturating_sub(self.last_update) < 900 && self.info.is_some() {
            return;
        }
        self.last_update = now;
        if let Some(info) = sys::system_info() {
            let used = info.total_memory_kib.saturating_sub(info.free_memory_kib) as f32
                / info.total_memory_kib.max(1) as f32;
            self.memory.push(used);
            if self.memory.len() > 80 {
                self.memory.remove(0);
            }
            let delta_switches = info.context_switches.saturating_sub(self.last_switches);
            let delta_ticks = info.ticks.saturating_sub(self.last_ticks).max(1);
            let load = (delta_switches as f32 / delta_ticks as f32).clamp(0.0, 1.0);
            self.activity.push(load);
            if self.activity.len() > 80 {
                self.activity.remove(0);
            }
            self.last_switches = info.context_switches;
            self.last_ticks = info.ticks;
            self.info = Some(info);
        }
        let mut buffer = [ProcessInfo {
            pid: 0,
            threads: 0,
            state: 0,
            name: [0; 24],
        }; 16];
        let count = sys::process_list(&mut buffer);
        self.processes.clear();
        self.processes.extend_from_slice(&buffer[..count]);
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let info = self.info.clone();
        let mut y = content.y + 18;
        let x = content.x + 20;
        surface.draw_text(x, y, "System Monitor", TITLE, palette.text);
        y += 30;
        let cards = [
            ("Uptime", info.as_ref().map(|i| format!("{} s", i.uptime_ms / 1000)).unwrap_or("-".into())),
            ("Processes", info.as_ref().map(|i| format!("{}", i.process_count)).unwrap_or("-".into())),
            ("Threads", info.as_ref().map(|i| format!("{}", i.thread_count)).unwrap_or("-".into())),
            ("Switches", info.as_ref().map(|i| format!("{}", i.context_switches)).unwrap_or("-".into())),
        ];
        let card_w = (content.w - 40 - 36) / 4;
        for (index, (label, value)) in cards.iter().enumerate() {
            let rect = Rect::new(x + index as i32 * (card_w + 12), y, card_w, 64);
            ui::draw_card(surface, rect, theme);
            surface.draw_text(rect.x + 12, rect.y + 10, label, CAPTION, palette.secondary);
            surface.draw_text(rect.x + 12, rect.y + 28, value, HEADLINE, palette.text);
        }
        y += 80;
        let graph_w = (content.w - 52) / 2;
        let graph_h = 130;
        for (index, (label, data, color)) in [
            ("Memory pressure", &self.memory, theme.accent_color()),
            ("Scheduler activity", &self.activity, SUCCESS),
        ]
        .iter()
        .enumerate()
        {
            let rect = Rect::new(x + index as i32 * (graph_w + 12), y, graph_w, graph_h);
            ui::draw_card(surface, rect, theme);
            surface.draw_text(rect.x + 12, rect.y + 10, label, BODY_MEDIUM, palette.text);
            let plot = Rect::new(rect.x + 12, rect.y + 36, rect.w - 24, rect.h - 48);
            surface.fill_rect(Rect::new(plot.x, plot.bottom() - 1, plot.w, 1), palette.hairline);
            let count = data.len().max(2);
            let mut previous: Option<(i32, i32)> = None;
            for (i, value) in data.iter().enumerate() {
                let px = plot.x + (i as f32 / (count - 1) as f32 * plot.w as f32) as i32;
                let py = plot.bottom() - (value.clamp(0.0, 1.0) * plot.h as f32) as i32;
                if let Some((ox, oy)) = previous {
                    surface.line(ox, oy, px, py, *color);
                }
                previous = Some((px, py));
            }
        }
        y += graph_h + 16;

        let table = Rect::new(x, y, content.w - 40, content.bottom() - y - 16);
        ui::draw_card(surface, table, theme);
        surface.draw_text(table.x + 12, table.y + 10, "Processes", BODY_MEDIUM, palette.text);
        let mut row_y = table.y + 36;
        surface.draw_text(table.x + 12, row_y, "PID", CAPTION, palette.tertiary);
        surface.draw_text(table.x + 80, row_y, "THREADS", CAPTION, palette.tertiary);
        surface.draw_text(table.x + 170, row_y, "NAME", CAPTION, palette.tertiary);
        row_y += 18;
        for process in &self.processes {
            if row_y > table.bottom() - 20 {
                break;
            }
            let name = core::str::from_utf8(&process.name).unwrap_or("?").trim_end_matches('\0');
            surface.draw_text(table.x + 12, row_y, &format!("{}", process.pid), BODY, palette.text);
            surface.draw_text(table.x + 80, row_y, &format!("{}", process.threads), BODY, palette.text);
            surface.draw_text(table.x + 170, row_y, name, BODY, palette.text);
            surface.fill_circle(table.right() - 24, row_y + 7, 3, SUCCESS);
            row_y += 24;
        }
    }
}

// ===========================================================================
// Text editor
// ===========================================================================

pub struct TextApp {
    path: String,
    text: String,
    cursor: usize,
    scroll: i32,
    dirty: bool,
    status: String,
}

impl TextApp {
    fn new(path: Option<&str>) -> Self {
        let mut app = Self {
            path: "/home/user/Documents/untitled.txt".to_string(),
            text: String::new(),
            cursor: 0,
            scroll: 0,
            dirty: false,
            status: "New file".to_string(),
        };
        if let Some(path) = path {
            app.load(path);
        }
        app
    }

    fn load(&mut self, path: &str) {
        self.path = path.to_string();
        match sys::read_file(path) {
            Some(data) => {
                self.text = String::from_utf8_lossy(&data).to_string();
                self.cursor = self.text.len();
                self.scroll = 0;
                self.dirty = false;
                self.status = format!("Opened {} ({} bytes)", path, data.len());
            }
            None => {
                self.text.clear();
                self.cursor = 0;
                self.dirty = false;
                self.status = format!("New file: {}", path);
            }
        }
    }

    fn display_name(&self) -> String {
        self.path.rsplit('/').next().unwrap_or("untitled").to_string()
    }

    fn save(&mut self) -> Action {
        if sys::write_file(&self.path, self.text.as_bytes()) {
            self.dirty = false;
            self.status = format!("Saved {} ({} bytes)", self.path, self.text.len());
            Action::Notify(format!("Saved {}", self.display_name()))
        } else {
            self.status = format!("Cannot save {}", self.path);
            Action::Notify("Save failed".to_string())
        }
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let toolbar = Rect::new(content.x, content.y, content.w, 42);
        surface.fill_rect(toolbar, palette.elevated);
        surface.fill_rect(Rect::new(toolbar.x, toolbar.bottom() - 1, toolbar.w, 1), palette.hairline);
        let save = Rect::new(toolbar.right() - 92, toolbar.y + 9, 80, 24);
        ui::draw_button(surface, save, "Save", true, theme, false, true);
        let new_button = Rect::new(toolbar.right() - 176, toolbar.y + 9, 76, 24);
        ui::draw_button(surface, new_button, "New", false, theme, false, true);
        let name = ui::truncate(surface, &format!("{}{}", self.display_name(), if self.dirty { " *" } else { "" }), BODY_MEDIUM, toolbar.w - 200);
        surface.draw_text(toolbar.x + 14, toolbar.y + 13, &name, BODY_MEDIUM, palette.text);

        let editor = Rect::new(content.x, toolbar.bottom(), content.w, content.bottom() - toolbar.bottom() - 22);
        let line_height = 18;
        let lines: Vec<&str> = self.text.split('\n').collect();
        let visible = (editor.h / line_height).max(1);
        let cursor_line = self.text[..self.cursor.min(self.text.len())].matches('\n').count() as i32;
        if cursor_line < self.scroll {
            self.scroll = cursor_line;
        }
        if cursor_line >= self.scroll + visible {
            self.scroll = cursor_line - visible + 1;
        }
        let text_x = editor.x + 46;
        for index in self.scroll..(self.scroll + visible).min(lines.len() as i32) {
            let y = editor.y + (index - self.scroll) * line_height + 8;
            let number = format!("{}", index + 1);
            surface.draw_text(editor.x + 16, y, &number, CAPTION, palette.tertiary);
            let line = lines[index as usize];
            let shown = ui::truncate(surface, line, MONO, editor.right() - text_x - 12);
            surface.draw_text(text_x, y, &shown, MONO, palette.text);
        }
        // Caret.
        let before = &self.text[..self.cursor.min(self.text.len())];
        let current_line = before.matches('\n').count() as i32;
        let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let column_text = &self.text[line_start..self.cursor.min(self.text.len())];
        let caret_x = text_x + surface.text_width(column_text, MONO);
        let caret_y = editor.y + (current_line - self.scroll) * line_height + 8;
        if caret_y >= editor.y && caret_y < editor.bottom() - 8 {
            surface.fill_rect(Rect::new(caret_x, caret_y + 1, 1, 16), theme.accent_color());
        }
        surface.fill_rect(Rect::new(content.x, content.bottom() - 22, content.w, 1), palette.hairline);
        surface.draw_text(content.x + 12, content.bottom() - 18, &self.status, CAPTION, palette.secondary);
    }

    fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.dirty = true;
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }
        let ctrl = event.mods & aster_abi::mods::CTRL != 0;
        match event.code {
            keys::BACKSPACE => {
                if self.cursor > 0 {
                    let mut index = self.cursor - 1;
                    while !self.text.is_char_boundary(index) && index > 0 {
                        index -= 1;
                    }
                    self.text.remove(index);
                    self.cursor = index;
                    self.dirty = true;
                }
            }
            keys::ENTER => self.insert('\n'),
            keys::TAB => {
                for _ in 0..4 {
                    self.insert(' ');
                }
            }
            keys::LEFT => {
                if self.cursor > 0 {
                    let mut index = self.cursor - 1;
                    while !self.text.is_char_boundary(index) && index > 0 {
                        index -= 1;
                    }
                    self.cursor = index;
                }
            }
            keys::RIGHT => {
                if self.cursor < self.text.len() {
                    let mut index = self.cursor + 1;
                    while !self.text.is_char_boundary(index) && index < self.text.len() {
                        index += 1;
                    }
                    self.cursor = index;
                }
            }
            keys::UP => {
                let before = &self.text[..self.cursor];
                if let Some(previous_newline) = before.trim_end_matches('\n').rfind('\n') {
                    self.cursor = previous_newline + 1;
                } else {
                    self.cursor = 0;
                }
            }
            keys::DOWN => {
                if let Some(next) = self.text[self.cursor..].find('\n') {
                    self.cursor += next + 1;
                }
            }
            keys::HOME => {
                let before = &self.text[..self.cursor];
                self.cursor = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            }
            keys::END => {
                self.cursor = self.text[self.cursor..].find('\n').map(|i| self.cursor + i).unwrap_or(self.text.len());
            }
            code if code == b's' as u32 && ctrl => return Some(self.save()),
            code if code == b'n' as u32 && ctrl => {
                self.path = "/home/user/Documents/untitled.txt".to_string();
                self.text.clear();
                self.cursor = 0;
                self.dirty = false;
                self.status = "New file".to_string();
            }
            code if code >= 32 && code < 127 => self.insert(code as u8 as char),
            _ => {}
        }
        None
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        if ly < 42 {
            if lx >= content.w - 92 && lx < content.w - 12 {
                return Some(self.save());
            }
            if lx >= content.w - 176 && lx < content.w - 100 {
                self.path = "/home/user/Documents/untitled.txt".to_string();
                self.text.clear();
                self.cursor = 0;
                self.dirty = false;
            }
            return None;
        }
        let line = ((ly - 42 - 8) / 18).max(0) as usize + self.scroll as usize;
        let mut index = 0;
        for (current, text_line) in self.text.split('\n').enumerate() {
            if current == line {
                self.cursor = index + text_line.len().min(text_line.len());
                break;
            }
            index += text_line.len() + 1;
        }
        None
    }
}

// ===========================================================================
// Notes
// ===========================================================================

pub struct NotesApp {
    notes: Vec<(String, String)>,
    selected: usize,
    buffer: String,
    dirty: bool,
    status: String,
}

impl NotesApp {
    fn new() -> Self {
        let mut app = Self {
            notes: Vec::new(),
            selected: 0,
            buffer: String::new(),
            dirty: false,
            status: String::new(),
        };
        app.reload();
        if app.notes.is_empty() {
            let path = "/home/user/notes/first-note.txt".to_string();
            let _ = sys::write_file(&path, b"Welcome to Notes.\n\nEverything you type is saved to the ramfs.\n");
            app.reload();
        }
        app.load_selected();
        app
    }

    fn reload(&mut self) {
        self.notes.clear();
        if let Some(entries) = sys::read_dir("/home/user/notes") {
            let mut names: Vec<(String, bool)> = entries.into_iter().map(|(name, is_dir, _)| (name, is_dir)).collect();
            names.sort();
            for (name, is_dir) in names {
                if !is_dir {
                    let path = format!("/home/user/notes/{}", name);
                    self.notes.push((name, path));
                }
            }
        }
        if self.selected >= self.notes.len() {
            self.selected = self.notes.len().saturating_sub(1);
        }
    }

    fn load_selected(&mut self) {
        if let Some((_, path)) = self.notes.get(self.selected) {
            self.buffer = sys::read_file(path)
                .map(|data| String::from_utf8_lossy(&data).to_string())
                .unwrap_or_default();
            self.dirty = false;
            self.status = format!("{} note(s)", self.notes.len());
        } else {
            self.buffer.clear();
        }
    }

    fn save(&mut self) -> Option<Action> {
        if let Some((name, path)) = self.notes.get(self.selected) {
            let name = name.clone();
            let path = path.clone();
            if sys::write_file(&path, self.buffer.as_bytes()) {
                self.dirty = false;
                self.status = format!("Saved {}", name);
                return Some(Action::Notify(format!("Saved note {}", name)));
            }
            self.status = "Save failed".to_string();
        }
        None
    }

    fn new_note(&mut self) -> Option<Action> {
        let stamp = sys::time_ms();
        let name = format!("note-{}.txt", stamp / 1000);
        let path = format!("/home/user/notes/{}", name);
        if sys::write_file(&path, b"New note\n") {
            self.reload();
            if let Some(index) = self.notes.iter().position(|(n, _)| *n == name) {
                self.selected = index;
            }
            self.load_selected();
            self.buffer = String::new();
            self.dirty = true;
            self.status = format!("Created {}", name);
        }
        None
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let sidebar = Rect::new(content.x, content.y, 190, content.h);
        surface.fill_rect(sidebar, palette.elevated);
        surface.draw_text(sidebar.x + 16, sidebar.y + 14, "Notes", TITLE, palette.text);
        let add = Rect::new(sidebar.right() - 40, sidebar.y + 12, 26, 24);
        surface.fill_rounded(add, 7, theme.accent_color());
        ui::draw_icon(surface, Icon::Plus, add.inset(6), theme.text_on_accent(), 1, theme);
        let mut y = sidebar.y + 52;
        for (index, (name, _)) in self.notes.iter().enumerate() {
            if y > sidebar.bottom() - 30 {
                break;
            }
            let row = Rect::new(sidebar.x + 8, y, sidebar.w - 16, 30);
            if index == self.selected {
                surface.fill_rounded(row, 7, theme.accent_color().with_alpha(40));
            }
            surface.draw_text(row.x + 10, row.y + 8, &ui::truncate(surface, name, BODY, row.w - 20), BODY, palette.text);
            y += 32;
        }
        let editor = Rect::new(sidebar.right(), content.y, content.right() - sidebar.right(), content.h);
        let toolbar = Rect::new(editor.x, editor.y, editor.w, 40);
        surface.fill_rect(toolbar, palette.surface);
        surface.fill_rect(Rect::new(toolbar.x, toolbar.bottom() - 1, toolbar.w, 1), palette.hairline);
        let save = Rect::new(toolbar.right() - 88, toolbar.y + 8, 76, 24);
        ui::draw_button(surface, save, "Save", true, theme, false, true);
        let delete = Rect::new(toolbar.right() - 168, toolbar.y + 8, 70, 24);
        ui::draw_button(surface, delete, "Delete", false, theme, false, true);

        let text_area = Rect::new(editor.x + 20, toolbar.bottom() + 16, editor.w - 40, editor.bottom() - toolbar.bottom() - 40);
        let mut y = text_area.y;
        for (index, line) in wrap_text(surface, &self.buffer, BODY, text_area.w).iter().take(((text_area.h / 18).max(1)) as usize).enumerate() {
            surface.draw_text(text_area.x, y, line, BODY, palette.text);
            y += 18;
            let _ = index;
        }
        if (sys::time_ms() / 500) % 2 == 0 && self.dirty {
            surface.fill_rect(Rect::new(text_area.x + 2, text_area.bottom() - 4, 80, 2), theme.accent_color());
        }
        surface.draw_text(editor.x + 20, editor.bottom() - 22, &self.status, CAPTION, palette.secondary);
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }
        let ctrl = event.mods & aster_abi::mods::CTRL != 0;
        match event.code {
            code if code == b's' as u32 && ctrl => return self.save(),
            code if code == b'n' as u32 && ctrl => return self.new_note(),
            keys::BACKSPACE => {
                self.buffer.pop();
                self.dirty = true;
            }
            keys::ENTER => {
                self.buffer.push('\n');
                self.dirty = true;
            }
            keys::DOWN => {
                if self.selected + 1 < self.notes.len() {
                    self.save();
                    self.selected += 1;
                    self.load_selected();
                }
            }
            keys::UP => {
                if self.selected > 0 {
                    self.save();
                    self.selected -= 1;
                    self.load_selected();
                }
            }
            code if code >= 32 && code < 127 => {
                self.buffer.push(code as u8 as char);
                self.dirty = true;
            }
            _ => {}
        }
        None
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        if lx < 190 {
            if ly >= 12 && ly <= 36 && lx >= 150 {
                return self.new_note();
            }
            let index = ((ly - 52) / 32).max(0) as usize;
            if ly >= 52 && index < self.notes.len() && self.selected != index {
                self.save();
                self.selected = index;
                self.load_selected();
            }
            return None;
        }
        if ly >= 8 && ly <= 32 {
            if lx >= content.w - 88 && lx <= content.w - 12 {
                return self.save();
            }
            if lx >= content.w - 168 && lx <= content.w - 98 {
                if let Some((name, path)) = self.notes.get(self.selected).cloned() {
                    if sys::unlink(&path) {
                        self.reload();
                        self.load_selected();
                        self.status = format!("Deleted {}", name);
                    }
                }
            }
        }
        None
    }
}

// ===========================================================================
// Calendar
// ===========================================================================

pub struct CalendarApp {
    year: i32,
    month: u8,
    day: u8,
    today: (i32, u8, u8),
}

impl CalendarApp {
    fn new() -> Self {
        let now = sys::rtc().unwrap_or(aster_abi::RtcTime {
            year: 2026,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            weekday: 0,
            reserved: 0,
        });
        Self {
            year: now.year as i32,
            month: now.month,
            day: now.day,
            today: (now.year as i32, now.month, now.day),
        }
    }

    fn tick(&mut self) {
        if let Some(now) = sys::rtc() {
            self.today = (now.year as i32, now.month, now.day);
        }
    }

    fn shift(&mut self, delta: i32) {
        let mut month = self.month as i32 + delta;
        let mut year = self.year;
        while month < 1 {
            month += 12;
            year -= 1;
        }
        while month > 12 {
            month -= 12;
            year += 1;
        }
        self.month = month as u8;
        self.year = year;
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        let header = Rect::new(content.x + 20, content.y + 18, content.w - 40, 36);
        surface.draw_text(header.x, header.y, &format!("{} {}", month_name(self.month), self.year), HEADLINE, palette.text);
        let prev = Rect::new(header.right() - 84, header.y + 2, 32, 26);
        let next = Rect::new(header.right() - 44, header.y + 2, 32, 26);
        for (rect, icon) in [(prev, Icon::ChevronLeft), (next, Icon::ChevronRight)] {
            surface.fill_rounded(rect, 7, palette.elevated);
            ui::draw_icon(surface, icon, rect.inset(8), palette.secondary, 1, theme);
        }

        let grid = Rect::new(content.x + 20, header.bottom() + 18, content.w - 40, content.bottom() - header.bottom() - 50);
        let cell_w = grid.w / 7;
        let cell_h = (grid.h / 7).max(30);
        for (index, label) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].iter().enumerate() {
            surface.draw_text(grid.x + index as i32 * cell_w + 8, grid.y, label, CAPTION, palette.tertiary);
        }
        let start_weekday = weekday_of(self.year, self.month, 1);
        let days = days_in_month(self.year, self.month);
        for day in 1..=days {
            let cell_index = start_weekday + day as i32 - 1;
            let col = cell_index % 7;
            let row = cell_index / 7;
            let cell = Rect::new(grid.x + col * cell_w + 4, grid.y + 22 + row * cell_h + 4, cell_w - 8, cell_h - 8);
            let is_today = (self.year, self.month, day) == self.today;
            if is_today {
                surface.fill_rounded(cell, 9, theme.accent_color());
            } else if day == self.day {
                surface.stroke_rounded(cell, 9, 1, palette.hairline);
            }
            let color = if is_today { theme.text_on_accent() } else { palette.text };
            surface.draw_text(cell.x + 9, cell.y + 7, &format!("{}", day), BODY, color);
        }
        if let Some(now) = sys::rtc() {
            surface.draw_text(content.x + 20, content.bottom() - 30, &format!("Today is {} {:02}, {} - {:02}:{:02}:{:02}", month_name(now.month), now.day, now.year, now.hour, now.minute, now.second), BODY, palette.secondary);
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect, _theme: &Theme) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        if ly >= 18 && ly <= 56 {
            if lx >= content.w - 84 && lx <= content.w - 56 {
                self.shift(-1);
            } else if lx >= content.w - 44 && lx <= content.w - 16 {
                self.shift(1);
            }
        }
        None
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }
        match event.code {
            keys::LEFT => self.shift(-1),
            keys::RIGHT => self.shift(1),
            keys::UP => self.shift(-12),
            keys::DOWN => self.shift(12),
            _ => {}
        }
        None
    }
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn weekday_of(year: i32, month: u8, day: u8) -> i32 {
    let (y, m) = if month < 3 {
        (year - 1, month as i32 + 12)
    } else {
        (year, month as i32)
    };
    let k = y % 100;
    let j = y / 100;
    let h = (day as i32 + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    // Zeller: 0 = Saturday. Convert to Monday = 0.
    (h + 5) % 7
}

fn month_name(month: u8) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        _ => "December",
    }
}

// ===========================================================================
// Photos / Wallpapers
// ===========================================================================

pub struct PhotosApp {
    selected: usize,
    thumbs: Vec<(usize, Vec<u32>, i32, i32)>,
    dark: bool,
}

impl PhotosApp {
    fn new(theme: &Theme) -> Self {
        let mut app = Self {
            selected: theme.wallpaper,
            thumbs: Vec::new(),
            dark: theme.dark,
        };
        app.build(theme);
        app
    }

    fn build(&mut self, theme: &Theme) {
        self.thumbs.clear();
        let width = 288;
        let height = 120;
        for index in 0..theme::WALLPAPER_COUNT {
            let mut pixels = vec![0u32; (width * height) as usize];
            theme::paint_wallpaper(&mut pixels, width, height, index, theme.dark);
            for pixel in &mut pixels {
                *pixel |= 0xFF00_0000;
            }
            self.thumbs.push((index, pixels, width, height));
        }
        self.dark = theme.dark;
    }

    fn tick(&mut self, theme: &Theme) {
        if theme.dark != self.dark {
            self.build(theme);
        }
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();
        surface.fill_rect(content, palette.surface);
        surface.draw_text(content.x + 20, content.y + 18, "Wallpapers", TITLE, palette.text);
        surface.draw_text(content.x + 20, content.y + 42, "Generated procedural wallpapers. Image file decoding is not implemented.", CAPTION, palette.secondary);
        let grid_x = content.x + 20;
        let grid_y = content.y + 70;
        let card_w = (content.w - 52) / 2;
        let card_h = 180;
        for (slot, (index, pixels, width, height)) in self.thumbs.iter().enumerate() {
            let col = slot as i32 % 2;
            let row = slot as i32 / 2;
            let card = Rect::new(grid_x + col * (card_w + 12), grid_y + row * (card_h + 12), card_w, card_h);
            if card.bottom() > content.bottom() {
                break;
            }
            ui::draw_card(surface, card, theme);
            let image = Rect::new(card.x + 10, card.y + 10, card.w - 20, 120);
            let previous = surface.set_clip(image);
            let draw_x = image.x + (image.w - *width).max(0) / 2;
            surface.blit_alpha(draw_x, image.y, *width, *height, pixels);
            surface.set_clip(previous);
            surface.stroke_rounded(image, 8, 1, palette.hairline);
            surface.draw_text(card.x + 12, card.bottom() - 30, theme::wallpaper_name(*index), BODY_MEDIUM, palette.text);
            if *index == theme.wallpaper {
                surface.draw_text(card.right() - 58, card.bottom() - 30, "Active", CAPTION, theme.accent_color());
            }
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect, _theme: &Theme) -> Option<Action> {
        let (lx, ly) = local(content, x, y);
        let grid_x = 20;
        let grid_y = 70;
        let card_w = (content.w - 52) / 2;
        let card_h = 180;
        let col = ((lx - grid_x) / (card_w + 12)).clamp(0, 1);
        let row = ((ly - grid_y) / (card_h + 12)).max(0);
        let index = (row * 2 + col) as usize;
        if index < self.thumbs.len() {
            self.selected = index;
            return Some(Action::SetWallpaper(index));
        }
        None
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value == 1 && event.code == keys::ENTER {
            return Some(Action::SetWallpaper(self.selected));
        }
        None
    }
}
