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

use aster_abi::{keys, mods, InputEvent, ProcessInfo, StorageDetails, SystemInfo};

use crate::font_data::{BODY, BODY_MEDIUM, CAPTION, DISPLAY, HEADLINE, MONO, SMALL, TITLE};

use crate::gfx::FloatMath;

use crate::gfx::{Color, Rect, Surface};

use crate::logo_data;

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

    SetAccent(usize),

    SetWallpaper(usize),

    SetAnimationSpeed(f32),

    SetReduceMotion(bool),

    SetBrightness(f32),

    SavePreferences,

    RestartServices,
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

            App::Files(app) => app.path.rsplit('/').next().unwrap_or("Files").to_string(),

            App::Settings(_) => "Settings".to_string(),

            App::Calculator(_) => "Calculator".to_string(),

            App::Monitor(_) => "Monitor".to_string(),

            App::Text(app) => app.display_name(),

            App::Notes(_) => "Notes".to_string(),

            App::Calendar(app) => format!("{} {}", month_name(app.month), app.year),

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

            App::Monitor(app) => app.mouse(x, y, content),

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

// ===========================================================================

// Terminal

// ===========================================================================

pub struct TerminalApp {
    lines: Vec<String>,

    input: String,

    history: Vec<String>,

    history_index: usize,

    cwd: String,

    last_pid: u64,
}

impl TerminalApp {
    fn new() -> Self {
        let mut lines = Vec::new();

        lines.push("Aster Terminal 0.3.0".to_string());

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

            last_pid: 0,
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
                    "  appearance            show the active appearance",
                    "  accent <name>        change the accent colour",
                    "  spawn <path>         start an isolated process",
                    "  wait [pid]           collect a child exit status",
                    "  isolation            run the isolation probes",
                    "  ipc                  run the channel IPC probe",
                    "  ipchup               forged handle and hangup checks",
                    "  systest              invalid syscall argument checks",
                    "  sync                 flush the persistent device",
                    "  mv <old> <new>       rename within a directory",
                    "  fstest               write/read/rename/delete benchmark",
                    "  services             show the settings service state",
                    "  svcfail              restart the settings service",
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
                            self.lines
                                .push(format!("{}{}", name, if is_dir { "/" } else { "" }));
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
                        self.lines
                            .push(format!("wrote {} bytes to {}", text.len(), path));
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

            "uname" => self
                .lines
                .push("Aster OS 0.3.0 x86_64 (aster kernel)".to_string()),

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

                    self.lines.push(format!(
                        "  {:>3}  {:>3}  {}",
                        process.pid, process.threads, name
                    ));
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

            "appearance" => self
                .lines
                .push("Appearance: Light (fixed in this build)".to_string()),

            "accent" => {
                if let Some(name) = args.first() {
                    if let Some(index) = theme::ACCENTS
                        .iter()
                        .position(|accent| accent.name.eq_ignore_ascii_case(name))
                    {
                        action = Some(Action::SetAccent(index));

                        self.lines
                            .push(format!("accent: {}", theme::ACCENTS[index].name));
                    } else {
                        self.lines.push(format!("unknown accent '{}'", name));
                    }
                } else {
                    let mut usage = String::from("usage: accent ");

                    for (index, accent) in theme::ACCENTS.iter().enumerate() {
                        if index > 0 {
                            usage.push('|');
                        }

                        usage.push_str(&accent.name.to_lowercase());
                    }

                    self.lines.push(usage);
                }
            }

            "spawn" => {
                if let Some(path) = args.first() {
                    let pid = sys::spawn(path);

                    if pid > 0 {
                        self.last_pid = pid as u64;

                        let message = format!("spawned pid {}: {}", pid, path);

                        sys::log(message.as_bytes());

                        self.lines.push(message);
                    } else {
                        self.lines.push(format!("spawn failed: {}", pid));
                    }
                } else {
                    self.lines.push("usage: spawn <path>".to_string());
                }
            }

            "wait" => {
                let pid = args
                    .first()
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(self.last_pid);

                if pid == 0 {
                    self.lines.push("usage: wait <pid>".to_string());
                } else {
                    let mut status = 0i64;

                    let child = sys::wait(pid, &mut status);

                    if child > 0 {
                        let message = format!("pid {} exited with status {}", child, status);

                        sys::log(message.as_bytes());

                        self.lines.push(message);
                    } else {
                        self.lines.push(format!("wait failed: {}", child));
                    }
                }
            }

            "isolation" => {
                let probe = "/home/user/isoprobe.elf";
                let fault = "/home/user/faultprobe.elf";

                let first = sys::spawn(probe);
                let second = sys::spawn(fault);

                self.lines
                    .push(format!("isolation: spawned pids {} and {}", first, second));

                for pid in [first, second] {
                    if pid <= 0 {
                        self.lines.push(format!("isolation: spawn failed: {}", pid));
                        continue;
                    }

                    let mut status = 0i64;

                    let child = sys::wait(pid as u64, &mut status);

                    let message = format!("isolation: pid {} status {}", child, status);

                    sys::log(message.as_bytes());

                    self.lines.push(message);
                }
            }

            "ipc" => {
                let Some((first, second)) = sys::channel_create() else {
                    self.lines.push("ipc: channel creation failed".to_string());

                    return None;
                };

                let pid = sys::spawn_with_handle("/home/user/ipcprobe.elf", second);

                if pid <= 0 {
                    sys::channel_close(first);
                    sys::channel_close(second);

                    self.lines.push(format!("ipc: spawn failed: {}", pid));

                    return None;
                }

                let sent = sys::channel_send(first, b"ping");
                let mut buffer = [0u8; 64];
                let received = sys::channel_recv(first, &mut buffer);

                let reply = if received > 0 {
                    String::from_utf8_lossy(&buffer[..received as usize]).to_string()
                } else {
                    format!("error {}", received)
                };

                let message = format!(
                    "ipc: sent {} bytes, received {} bytes: {}",
                    sent, received, reply
                );

                sys::log(message.as_bytes());

                self.lines.push(message);

                sys::channel_close(first);
                sys::channel_close(second);

                let mut status = 0i64;

                let child = sys::wait(pid as u64, &mut status);
                let message = format!("ipc: pid {} status {}", child, status);

                sys::log(message.as_bytes());

                self.lines.push(message);
            }

            "ipchup" => {
                let forged = sys::channel_send(15, b"x");

                let message = format!("ipc: forged handle {}", forged);

                sys::log(message.as_bytes());

                self.lines.push(message);

                let Some((first, second)) = sys::channel_create() else {
                    self.lines.push("ipc: channel creation failed".to_string());

                    return None;
                };

                let pid = sys::spawn_with_handle("/home/user/isoprobe.elf", second);

                sys::channel_close(second);

                let mut buffer = [0u8; 16];
                let received = sys::channel_recv(first, &mut buffer);

                let message = format!("ipc: hangup {}", received);

                sys::log(message.as_bytes());

                self.lines.push(message);

                sys::channel_close(first);

                if pid > 0 {
                    let mut status = 0i64;

                    let child = sys::wait(pid as u64, &mut status);
                    let message = format!("ipc: pid {} status {}", child, status);

                    sys::log(message.as_bytes());

                    self.lines.push(message);
                }
            }

            "systest" => {
                let stat = sys::raw(aster_abi::sys::STAT, 1, 0, 0);
                let log = sys::raw(aster_abi::sys::LOG, 0xFFFF_FFFF_8000_0000, 4, 0);
                let open = sys::raw(aster_abi::sys::OPEN, 0xDEAD_0000, 1, 0);
                let wait = sys::raw(aster_abi::sys::WAIT, 0, 0, 0);

                let message = format!(
                    "systest: stat {} log {} open {} wait {}",
                    stat, log, open, wait
                );

                sys::log(message.as_bytes());

                self.lines.push(message);
            }

            "sync" => {
                let result = sys::sync();
                self.lines.push(if result >= 0 {
                    "sync: persistent device flushed".to_string()
                } else {
                    format!("sync failed: {}", result)
                });
            }

            "mv" => {
                if args.len() >= 2 {
                    let result = sys::rename(args[0], args[1]);
                    if result >= 0 {
                        self.lines
                            .push(format!("renamed {} to {}", args[0], args[1]));
                        let _ = sys::sync();
                    } else {
                        self.lines.push(format!("rename failed: {}", result));
                    }
                } else {
                    self.lines.push("usage: mv <old> <new>".to_string());
                }
            }

            "fstest" => {
                const SIZE: usize = 64 * 1024;
                let mut payload = vec![0u8; SIZE];
                for (index, byte) in payload.iter_mut().enumerate() {
                    *byte = (index % 251) as u8;
                }

                let start = sys::time_ms();
                let written = sys::write_file("/home/user/FSTEST.TXT", &payload);
                let write_ms = sys::time_ms().saturating_sub(start);

                let start = sys::time_ms();
                let read_back = sys::read_file("/home/user/FSTEST.TXT");
                let read_ms = sys::time_ms().saturating_sub(start);

                let read_ok = read_back
                    .as_ref()
                    .map(|data| {
                        data.len() == SIZE && data[0] == 0 && data[SIZE - 1] == payload[SIZE - 1]
                    })
                    .unwrap_or(false);

                let stored_size = sys::stat("/home/user/FSTEST.TXT")
                    .map(|stat| stat.size)
                    .unwrap_or(0);
                let first_bad = read_back
                    .as_ref()
                    .and_then(|data| {
                        data.iter()
                            .zip(payload.iter())
                            .position(|(stored, expected)| stored != expected)
                    })
                    .map(|index| index as i64)
                    .unwrap_or(-1);

                let renamed = sys::rename("/home/user/FSTEST.TXT", "/home/user/FSTEST2.TXT") >= 0;
                let renamed_ok = renamed
                    && sys::stat("/home/user/FSTEST2.TXT")
                        .map(|stat| stat.size == SIZE as u64)
                        .unwrap_or(false);

                let deleted = sys::unlink("/home/user/FSTEST2.TXT");
                let _ = sys::sync();

                let message = format!(
                    "fstest: write 64 KiB {} ms (size {}), read {} ms ({} B, first bad {}), rename {} , delete {}",
                    write_ms,
                    stored_size,
                    read_ms,
                    read_back.as_ref().map(|data| data.len()).unwrap_or(0),
                    first_bad,
                    if renamed_ok { "ok" } else { "FAILED" },
                    if deleted { "ok" } else { "FAILED" }
                );

                if written && read_ok && renamed_ok && deleted {
                    sys::log(message.as_bytes());
                }

                self.lines.push(message);
            }

            "services" => {
                let mut buffer: [ProcessInfo; 16] = unsafe { core::mem::zeroed() };
                let count = sys::process_list(&mut buffer);
                let mut found = None;
                for process in &buffer[..count] {
                    let name = core::str::from_utf8(&process.name)
                        .unwrap_or("?")
                        .trim_end_matches('\0');
                    if name == "configd" {
                        found = Some((process.pid, process.state));
                    }
                }
                let line = match found {
                    Some((pid, 0)) => format!("configd: running (pid {})", pid),
                    Some((pid, _)) => format!("configd: exited, awaiting restart (pid {})", pid),
                    None => "configd: not running".to_string(),
                };
                sys::log(line.as_bytes());
                self.lines.push(line);
            }

            "svcfail" => {
                self.lines
                    .push("Requested a settings-service restart".to_string());
                action = Some(Action::RestartServices);
            }

            "open" => match args.first().copied() {
                Some("files") => action = Some(Action::Launch(AppKind::Files)),

                Some("settings") => action = Some(Action::Launch(AppKind::Settings)),

                Some("monitor") => action = Some(Action::Launch(AppKind::Monitor)),

                Some("notes") => action = Some(Action::Launch(AppKind::Notes)),

                Some("calc") | Some("calculator") => {
                    action = Some(Action::Launch(AppKind::Calculator))
                }

                Some("text") => action = Some(Action::Launch(AppKind::Text)),

                Some("calendar") => action = Some(Action::Launch(AppKind::Calendar)),

                Some("photos") | Some("wallpapers") => {
                    action = Some(Action::Launch(AppKind::Photos))
                }

                _ => self.lines.push(
                    "usage: open <files|settings|monitor|notes|calc|text|calendar|photos>"
                        .to_string(),
                ),
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

        surface.draw_text(
            content.x + padding,
            input_line,
            &prompt,
            MONO,
            theme.accent_color(),
        );

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

        surface.draw_text(
            content.x + padding + prompt_width,
            input_line,
            &shown,
            MONO,
            palette.text,
        );

        if (sys::time_ms() / 500) % 2 == 0 {
            let caret = content.x + padding + prompt_width + surface.text_width(&shown, MONO) + 1;

            surface.fill_rect(
                Rect::new(caret, input_line + 1, 2, 15),
                theme.accent_color(),
            );
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
    all_entries: Vec<Entry>,
    entries: Vec<Entry>,
    selected: usize,
    scroll: usize,
    back: Vec<String>,
    forward: Vec<String>,
    last_click: (usize, u64),
    status: String,
    sort: SortMode,
    ascending: bool,
    search: String,
    search_focus: bool,
    rename: Option<String>,
    info: Option<Vec<String>>,
    confirm_delete: bool,
    clipboard: Option<Clipboard>,
    breadcrumb_hits: Vec<(Rect, String)>,
}

#[derive(Clone)]
struct Entry {
    name: String,
    is_dir: bool,
    size: u64,
}

#[derive(Clone)]
struct Clipboard {
    path: String,
    name: String,
    is_dir: bool,
    cut: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Name,
    Size,
    Kind,
}

impl SortMode {
    fn label(self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::Size => "Size",
            SortMode::Kind => "Kind",
        }
    }
}

const FILE_ACTIONS: [(&str, i32); 8] = [
    ("New", 40),
    ("Delete", 50),
    ("Rename", 58),
    ("Info", 38),
    ("Copy", 44),
    ("Move", 44),
    ("Paste", 48),
    ("Sort Name", 82),
];

fn join_path(directory: &str, name: &str) -> String {
    if directory == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", directory, name)
    }
}

fn parent_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
        .unwrap_or("/")
        .to_string()
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KiB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

fn copy_tree(source: &str, destination: &str, depth: u32) -> bool {
    if depth > 8 || !sys::mkdir(destination) {
        return false;
    }
    let Some(entries) = sys::read_dir(source) else {
        return false;
    };
    for (name, is_dir, _) in entries {
        let from = join_path(source, &name);
        let to = join_path(destination, &name);
        if is_dir {
            if !copy_tree(&from, &to, depth + 1) {
                return false;
            }
        } else {
            let Some(data) = sys::read_file(&from) else {
                return false;
            };
            if !sys::write_file(&to, &data) {
                return false;
            }
        }
    }
    true
}

impl FilesApp {
    fn new() -> Self {
        let mut app = Self {
            path: "/home/user".to_string(),
            all_entries: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            back: Vec::new(),
            forward: Vec::new(),
            last_click: (usize::MAX, 0),
            status: String::new(),
            sort: SortMode::Name,
            ascending: true,
            search: String::new(),
            search_focus: false,
            rename: None,
            info: None,
            confirm_delete: false,
            clipboard: None,
            breadcrumb_hits: Vec::new(),
        };

        // The seed volume has these locations; older volumes may not.
        let _ = sys::mkdir("/home/user/DOWNLOADS");
        let _ = sys::mkdir("/home/user/DESKTOP");

        app.reload();

        app
    }

    fn reload(&mut self) {
        self.all_entries.clear();

        match sys::read_dir(&self.path) {
            Some(entries) => {
                for (name, is_dir, _) in entries {
                    let size = if is_dir {
                        0
                    } else {
                        sys::stat(&join_path(&self.path, &name))
                            .map(|stat| stat.size)
                            .unwrap_or(0)
                    };
                    self.all_entries.push(Entry { name, is_dir, size });
                }
                self.sort_entries();
                self.apply_filter();
                self.status = format!("{} items", self.entries.len());
            }
            None => {
                self.all_entries.clear();
                self.entries.clear();
                self.status = format!("Cannot open {}", self.path);
            }
        }

        self.selected = 0;
        self.scroll = 0;
    }

    fn sort_entries(&mut self) {
        let mode = self.sort;
        let ascending = self.ascending;
        self.all_entries.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                return b.is_dir.cmp(&a.is_dir);
            }
            let order = match mode {
                SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortMode::Size => a.size.cmp(&b.size).then(a.name.cmp(&b.name)),
                SortMode::Kind => a
                    .name
                    .rsplit_once('.')
                    .map(|(_, ext)| ext.to_lowercase())
                    .cmp(&b.name.rsplit_once('.').map(|(_, ext)| ext.to_lowercase()))
                    .then(a.name.cmp(&b.name)),
            };
            if ascending {
                order
            } else {
                order.reverse()
            }
        });
    }

    fn apply_filter(&mut self) {
        let needle = self.search.to_lowercase();
        self.entries = self
            .all_entries
            .iter()
            .filter(|entry| needle.is_empty() || entry.name.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    fn navigate(&mut self, path: String) {
        if path != self.path {
            self.back.push(self.path.clone());
            self.forward.clear();
            self.path = path;
            self.search.clear();
            self.confirm_delete = false;
            self.reload();
        }
    }

    fn go_back(&mut self) {
        if let Some(previous) = self.back.pop() {
            self.forward.push(self.path.clone());
            self.path = previous;
            self.search.clear();
            self.reload();
        }
    }

    fn go_forward(&mut self) {
        if let Some(next) = self.forward.pop() {
            self.back.push(self.path.clone());
            self.path = next;
            self.search.clear();
            self.reload();
        }
    }

    fn selections_path(&self) -> Option<String> {
        self.entries
            .get(self.selected)
            .map(|entry| join_path(&self.path, &entry.name))
    }

    fn open_selected(&mut self) -> Option<Action> {
        let entry = self.entries.get(self.selected)?.clone();
        let path = join_path(&self.path, &entry.name);
        if entry.is_dir {
            self.navigate(path);
            None
        } else {
            Some(Action::OpenPath(path))
        }
    }

    fn unique_name(&self, base: &str) -> String {
        if sys::stat(&join_path(&self.path, base)).is_none() {
            return base.to_string();
        }
        let (stem, extension) = match base.rsplit_once('.') {
            Some((stem, extension)) => (stem, format!(".{}", extension)),
            None => (base, String::new()),
        };
        for index in 1..100u32 {
            let suffix = format!("-{}", index);
            let limit = 8usize.saturating_sub(suffix.len()).max(1);
            let candidate = format!("{}{}{}", &stem[..stem.len().min(limit)], suffix, extension);
            if sys::stat(&join_path(&self.path, &candidate)).is_none() {
                return candidate;
            }
        }
        base.to_string()
    }

    fn create_folder(&mut self) {
        let name = self.unique_name("NEWDIR");
        let path = join_path(&self.path, &name);
        if sys::mkdir(&path) {
            let _ = sys::sync();
            self.reload();
            if let Some(index) = self.entries.iter().position(|entry| entry.name == name) {
                self.selected = index;
            }
            sys::log(format!("files: created {}", name).as_bytes());
            self.status = format!("Created {}", name);
        } else {
            self.status = "Could not create the folder".to_string();
        }
    }

    fn delete_selected(&mut self) {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return;
        };
        if !self.confirm_delete {
            self.confirm_delete = true;
            self.status = format!("Click Delete again to remove {}", entry.name);
            return;
        }
        let path = join_path(&self.path, &entry.name);
        if sys::unlink(&path) {
            let _ = sys::sync();
            self.reload();
            sys::log(format!("files: deleted {}", entry.name).as_bytes());
            self.status = format!("Deleted {}", entry.name);
        } else {
            self.status = format!("Could not delete {}", entry.name);
        }
        self.confirm_delete = false;
    }

    fn begin_rename(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            self.rename = Some(entry.name.clone());
            self.confirm_delete = false;
        }
    }

    fn commit_rename(&mut self) {
        let Some(buffer) = self.rename.take() else {
            return;
        };
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return;
        };
        let name = buffer.trim();
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            self.status = "Invalid name".to_string();
            return;
        }
        let old_path = join_path(&self.path, &entry.name);
        let new_path = join_path(&self.path, name);
        if new_path == old_path {
            return;
        }
        if sys::rename(&old_path, &new_path) >= 0 {
            let _ = sys::sync();
            self.reload();
            if let Some(index) = self
                .entries
                .iter()
                .position(|entry| entry.name.eq_ignore_ascii_case(name))
            {
                self.selected = index;
            }
            sys::log(format!("files: renamed {} to {}", entry.name, name).as_bytes());
            self.status = format!("Renamed to {}", name);
        } else {
            self.status = "Rename failed (name may exist or exceed 8.3)".to_string();
        }
    }

    fn show_info(&mut self) {
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        let path = join_path(&self.path, &entry.name);
        let Some(stat) = sys::stat(&path) else {
            self.status = "Information unavailable".to_string();
            return;
        };
        let stored = if path.starts_with("/home/user") {
            "Persistent home"
        } else {
            "In-memory system"
        };
        self.info = Some(alloc::vec![
            entry.name.clone(),
            format!("Path: {}", path),
            format!("Type: {}", if entry.is_dir { "Folder" } else { "Document" }),
            format!("Size: {}", format_size(stat.size)),
            format!("Inode: {}", stat.inode),
            format!("Mode: {:o}", stat.mode),
            format!("Storage: {}", stored),
            "Enter or click to close".to_string(),
        ]);
    }

    fn copy_selected(&mut self, cut: bool) {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return;
        };
        let path = join_path(&self.path, &entry.name);
        self.status = if cut {
            format!("Ready to move {}", entry.name)
        } else {
            format!("Copied {} to the clipboard", entry.name)
        };
        self.clipboard = Some(Clipboard {
            path,
            name: entry.name,
            is_dir: entry.is_dir,
            cut,
        });
        self.confirm_delete = false;
    }

    fn paste(&mut self) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let target_name = self.unique_name(&clipboard.name);
        let target = join_path(&self.path, &target_name);
        let copied = if clipboard.is_dir {
            copy_tree(&clipboard.path, &target, 0)
        } else {
            match sys::read_file(&clipboard.path) {
                Some(data) => sys::write_file(&target, &data),
                None => false,
            }
        };
        if !copied {
            self.status = "Paste failed".to_string();
            return;
        }
        if clipboard.cut {
            let _ = sys::unlink(&clipboard.path);
            self.clipboard = None;
        }
        let _ = sys::sync();
        self.reload();
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.name == target_name)
        {
            self.selected = index;
        }
        sys::log(
            format!(
                "files: {} {} to {}",
                if clipboard.cut { "moved" } else { "copied" },
                clipboard.name,
                target_name
            )
            .as_bytes(),
        );
        self.status = if clipboard.cut {
            format!("Moved to {}", target_name)
        } else {
            format!("Pasted as {}", target_name)
        };
    }

    fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Kind,
            SortMode::Kind => {
                self.ascending = !self.ascending;
                SortMode::Name
            }
        };
        self.sort_entries();
        self.apply_filter();
        self.status = format!(
            "Sorted by {} {}",
            self.sort.label(),
            if self.ascending {
                "ascending"
            } else {
                "descending"
            }
        );
    }

    fn action_rect(toolbar: Rect, index: usize) -> Rect {
        let mut x = toolbar.x + 10;
        for (position, (_, width)) in FILE_ACTIONS.iter().enumerate() {
            if position == index {
                return Rect::new(x, toolbar.y + 40, *width, 24);
            }
            x += width + 6;
        }
        Rect::new(x, toolbar.y + 40, 0, 0)
    }

    fn search_rect(toolbar: Rect) -> Rect {
        Rect::new(
            (toolbar.right() - 142).max(toolbar.x + 10),
            toolbar.y + 40,
            132,
            24,
        )
    }

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();

        surface.fill_rect(content, palette.surface);

        let sidebar = Rect::new(content.x, content.y, 176, content.h);
        surface.fill_rect(sidebar, palette.sidebar);

        let places = [
            ("Home", "/home/user", Icon::Home),
            ("Documents", "/home/user/DOCS", Icon::Folder),
            ("Downloads", "/home/user/DOWNLOADS", Icon::Folder),
            ("Desktop", "/home/user/DESKTOP", Icon::Monitor),
            ("Notes", "/home/user/NOTES", Icon::Notes),
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
                if self.path == path {
                    theme.accent_color()
                } else {
                    palette.secondary
                },
                1,
                theme,
            );
            surface.draw_text(
                row.x + 32,
                row.y + 8,
                label,
                BODY,
                if self.path == path {
                    palette.text
                } else {
                    palette.secondary
                },
            );
            y += 32;
        }

        surface.fill_rect(
            Rect::new(sidebar.right() - 1, sidebar.y, 1, sidebar.h),
            palette.hairline,
        );

        let toolbar = Rect::new(
            sidebar.right(),
            content.y,
            content.right() - sidebar.right(),
            72,
        );
        surface.fill_rect(toolbar, palette.surface);
        surface.fill_rect(
            Rect::new(toolbar.x, toolbar.bottom() - 1, toolbar.w, 1),
            palette.hairline,
        );

        for (index, icon) in [
            Icon::ChevronLeft,
            Icon::ChevronRight,
            Icon::Up,
            Icon::Refresh,
        ]
        .iter()
        .enumerate()
        {
            let rect = Rect::new(toolbar.x + 10 + index as i32 * 32, toolbar.y + 8, 26, 24);
            let enabled = match index {
                0 => !self.back.is_empty(),
                1 => !self.forward.is_empty(),
                2 => self.path != "/",
                _ => true,
            };
            surface.fill_rounded(
                rect,
                7,
                if enabled {
                    palette.elevated
                } else {
                    palette.surface
                },
            );
            ui::draw_icon(
                surface,
                *icon,
                rect.inset(6),
                if enabled {
                    palette.secondary
                } else {
                    palette.tertiary
                },
                1,
                theme,
            );
        }

        // Breadcrumbs.
        self.breadcrumb_hits.clear();
        let mut crumbs: Vec<(String, String)> = alloc::vec![("Aster".to_string(), "/".to_string())];
        let mut accumulated = String::new();
        for component in self.path.split('/').filter(|part| !part.is_empty()) {
            accumulated.push('/');
            accumulated.push_str(component);
            crumbs.push((component.to_string(), accumulated.clone()));
        }
        if crumbs.len() > 4 {
            let last = crumbs.split_off(crumbs.len() - 3);
            let mut shortened = alloc::vec![crumbs[0].clone(), ("...".to_string(), String::new())];
            shortened.extend(last);
            crumbs = shortened;
        }
        let mut x = toolbar.x + 146;
        let right_limit = toolbar.right() - 12;
        for (index, (label, path)) in crumbs.iter().enumerate() {
            let width = surface.text_width(label, BODY).min(110);
            if x + width > right_limit {
                break;
            }
            let hit = Rect::new(x - 3, toolbar.y + 10, width + 6, 20);
            if !path.is_empty() && self.path == *path {
                surface.fill_rounded(hit, 5, theme.accent_color().with_alpha(28));
            }
            ui::draw_text_truncated(
                surface,
                x,
                toolbar.y + 14,
                label,
                BODY,
                if self.path == *path {
                    palette.text
                } else {
                    palette.secondary
                },
                110,
            );
            if !path.is_empty() {
                self.breadcrumb_hits.push((hit, path.clone()));
            }
            x += width + 8;
            if index + 1 < crumbs.len() {
                surface.draw_text(x - 6, toolbar.y + 14, ">", SMALL, palette.tertiary);
                x += 10;
            }
        }

        let has_selection = !self.entries.is_empty();
        for (index, (label, _)) in FILE_ACTIONS.iter().enumerate() {
            let rect = Self::action_rect(toolbar, index);
            if rect.x + rect.w > toolbar.right() - 150 {
                break;
            }
            let enabled = match index {
                0 => true,
                1 | 2 | 3 | 4 | 5 => has_selection,
                6 => self.clipboard.is_some(),
                _ => true,
            };
            let label = if index == 1 && self.confirm_delete {
                "Confirm"
            } else if index == 7 {
                self.sort.label()
            } else {
                label
            };
            ui::draw_button(surface, rect, label, index == 0, theme, false, enabled);
        }

        let search = Self::search_rect(toolbar);
        surface.fill_rounded(
            search,
            7,
            if self.search_focus {
                palette.elevated
            } else {
                palette.surface
            },
        );
        surface.stroke_rounded(
            search,
            7,
            1,
            if self.search_focus {
                theme.accent_color()
            } else {
                palette.hairline
            },
        );
        let shown = if self.search.is_empty() {
            "Search"
        } else {
            &self.search
        };
        ui::draw_text_truncated(
            surface,
            search.x + 8,
            search.y + 4,
            shown,
            CAPTION,
            if self.search.is_empty() {
                palette.tertiary
            } else {
                palette.text
            },
            search.w - 16,
        );

        let list = Rect::new(
            toolbar.x,
            toolbar.bottom(),
            toolbar.w,
            content.bottom() - toolbar.bottom(),
        );
        let row_height = 32;
        let visible = ((list.h - 22) / row_height).max(1) as usize;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + visible {
            self.scroll = self.selected - visible + 1;
        }

        if self.entries.is_empty() {
            let message = if self.search.is_empty() {
                "This folder is empty"
            } else {
                "No matching items"
            };
            surface.draw_text_centered(
                list,
                list.y + list.h / 2 - 8,
                message,
                BODY,
                palette.tertiary,
            );
        }

        let mut y = list.y + 4;
        for (index, entry) in self
            .entries
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(visible)
        {
            let row = Rect::new(list.x + 8, y, list.w - 16, row_height - 2);
            if index == self.selected {
                surface.fill_rounded(row, 6, theme.accent_color());
            }
            ui::draw_file_icon(
                surface,
                Rect::new(row.x + 8, row.y + 7, 18, 18),
                entry.is_dir,
                index == self.selected,
                theme,
            );
            let text_color = if index == self.selected {
                Color::rgb(255, 255, 255)
            } else {
                palette.text
            };
            ui::draw_text_truncated(
                surface,
                row.x + 34,
                row.y + 8,
                &entry.name,
                BODY,
                text_color,
                row.w - 150,
            );
            let detail = if entry.is_dir {
                "Folder".to_string()
            } else {
                format_size(entry.size)
            };
            surface.draw_text(
                row.right() - 118,
                row.y + 9,
                &detail,
                CAPTION,
                if index == self.selected {
                    Color::rgba(255, 255, 255, 210)
                } else {
                    palette.tertiary
                },
            );
            if entry.is_dir {
                surface.draw_text(
                    row.right() - 16,
                    row.y + 8,
                    ">",
                    SMALL,
                    if index == self.selected {
                        Color::rgb(255, 255, 255)
                    } else {
                        palette.tertiary
                    },
                );
            }
            y += row_height;
        }

        surface.fill_rect(
            Rect::new(list.x, list.bottom() - 21, list.w, 1),
            palette.hairline,
        );
        let status = if !self.search.is_empty() {
            format!(
                "{} ({} of {} items)",
                self.status,
                self.entries.len(),
                self.all_entries.len()
            )
        } else {
            self.status.clone()
        };
        ui::draw_text_truncated(
            surface,
            list.x + 10,
            list.bottom() - 17,
            &status,
            CAPTION,
            palette.tertiary,
            list.w - 20,
        );

        if self.search_focus && (sys::time_ms() / 500) % 2 == 0 {
            let caret = search.x
                + 8
                + surface.text_width(
                    &ui::truncate(surface, &self.search, CAPTION, search.w - 16),
                    CAPTION,
                )
                + 1;
            surface.fill_rect(Rect::new(caret, search.y + 5, 2, 14), theme.accent_color());
        }

        if let Some(lines) = &self.info {
            surface.fill_rect(content, Color::rgba(0, 0, 0, 60));
            let card = Rect::new(
                content.x + (content.w - 420) / 2,
                content.y + (content.h - 230) / 2,
                420,
                230,
            );
            surface.fill_rounded(card, 14, palette.surface);
            surface.stroke_rounded(card, 14, 1, palette.hairline);
            surface.draw_text(card.x + 20, card.y + 16, "Information", TITLE, palette.text);
            let mut line_y = card.y + 52;
            for (index, line) in lines.iter().enumerate() {
                surface.draw_text(
                    card.x + 20,
                    line_y,
                    line,
                    if index == 0 { BODY_MEDIUM } else { BODY },
                    if index == 0 {
                        palette.text
                    } else {
                        palette.secondary
                    },
                );
                line_y += 22;
            }
        }

        if let Some(buffer) = &self.rename {
            surface.fill_rect(content, Color::rgba(0, 0, 0, 60));
            let card = Rect::new(
                content.x + (content.w - 380) / 2,
                content.y + (content.h - 130) / 2,
                380,
                130,
            );
            surface.fill_rounded(card, 14, palette.surface);
            surface.stroke_rounded(card, 14, 1, palette.hairline);
            surface.draw_text(card.x + 20, card.y + 16, "Rename", TITLE, palette.text);
            let field = Rect::new(card.x + 20, card.y + 54, card.w - 40, 28);
            surface.fill_rounded(field, 7, palette.elevated);
            surface.stroke_rounded(field, 7, 1, theme.accent_color());
            ui::draw_text_truncated(
                surface,
                field.x + 8,
                field.y + 5,
                buffer,
                BODY,
                palette.text,
                field.w - 16,
            );
            surface.draw_text(
                card.x + 20,
                card.y + 92,
                "Enter to rename - Esc to cancel",
                CAPTION,
                palette.tertiary,
            );
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect) -> Option<Action> {
        let (lx, ly) = local(content, x, y);

        if self.info.is_some() {
            self.info = None;
            return None;
        }
        if self.rename.is_some() {
            // Clicking outside the modal cancels; inside keeps editing.
            if lx < (content.w - 380) / 2 || lx > (content.w + 380) / 2 {
                self.rename = None;
            }
            return None;
        }

        if lx < 176 {
            let places = [
                "/home/user",
                "/home/user/DOCS",
                "/home/user/DOWNLOADS",
                "/home/user/DESKTOP",
                "/home/user/NOTES",
                "/system",
                "/var/log",
                "/",
            ];
            let index = ((ly - 16) / 32).max(0) as usize;
            if ly >= 16 && index < places.len() {
                self.navigate(places[index].to_string());
            }
            return None;
        }

        let toolbar = Rect::new(176, 0, content.w - 176, 72);

        if ly < 36 {
            for index in 0..4i32 {
                let rect = Rect::new(10 + index * 32, 8, 26, 24);
                if rect.contains(lx, ly) {
                    match index {
                        0 => self.go_back(),
                        1 => self.go_forward(),
                        2 => {
                            if self.path != "/" {
                                self.navigate(parent_path(&self.path));
                            }
                        }
                        _ => self.reload(),
                    }
                    return None;
                }
            }
            for (rect, path) in &self.breadcrumb_hits {
                if rect.contains(lx, ly) {
                    self.navigate(path.clone());
                    return None;
                }
            }
            return None;
        }

        if ly < 72 {
            for index in 0..FILE_ACTIONS.len() {
                let rect = Self::action_rect(toolbar, index);
                if rect.w == 0 || !rect.contains(lx, ly) {
                    continue;
                }
                match index {
                    0 => self.create_folder(),
                    1 => self.delete_selected(),
                    2 => self.begin_rename(),
                    3 => self.show_info(),
                    4 => self.copy_selected(false),
                    5 => self.copy_selected(true),
                    6 => self.paste(),
                    7 => self.cycle_sort(),
                    _ => {}
                }
                return None;
            }
            let search = Self::search_rect(toolbar);
            if search.contains(lx, ly) {
                self.search_focus = true;
                self.confirm_delete = false;
                return None;
            }
            self.search_focus = false;
            return None;
        }

        self.search_focus = false;

        let list_top = 72;
        let index = self.scroll + ((ly - list_top - 4) / 32).max(0) as usize;
        if index < self.entries.len() {
            let now = sys::time_ms();
            if self.selected == index && now.saturating_sub(self.last_click.1) < 450 {
                return self.open_selected();
            }
            self.selected = index;
            self.confirm_delete = false;
            self.last_click = (index, now);
        }

        None
    }

    fn key(&mut self, event: &InputEvent) -> Option<Action> {
        if event.value != 1 {
            return None;
        }

        if self.info.is_some() {
            self.info = None;
            return None;
        }

        if let Some(buffer) = &mut self.rename {
            match event.code {
                keys::ENTER => self.commit_rename(),
                keys::ESC => self.rename = None,
                keys::BACKSPACE => {
                    buffer.pop();
                }
                code if code >= 32 && code < 127 => {
                    if buffer.len() < 48 {
                        buffer.push(code as u8 as char);
                    }
                }
                _ => {}
            }
            return None;
        }

        if self.search_focus {
            match event.code {
                keys::BACKSPACE => {
                    self.search.pop();
                    self.apply_filter();
                }
                keys::ESC => self.search_focus = false,
                keys::ENTER => self.search_focus = false,
                code if code >= 32 && code < 127 => {
                    if self.search.len() < 32 {
                        self.search.push(code as u8 as char);
                        self.apply_filter();
                    }
                }
                _ => {}
            }
            return None;
        }

        if event.mods & mods::CTRL != 0 {
            match event.code {
                code if code == 'n' as u32 => {
                    self.create_folder();
                    return None;
                }
                code if code == 'c' as u32 => {
                    self.copy_selected(false);
                    return None;
                }
                code if code == 'x' as u32 => {
                    self.copy_selected(true);
                    return None;
                }
                code if code == 'v' as u32 => {
                    self.paste();
                    return None;
                }
                code if code == 'i' as u32 => {
                    self.show_info();
                    return None;
                }
                code if code == 'f' as u32 => {
                    self.search_focus = true;
                    return None;
                }
                code if code == 'r' as u32 => {
                    self.reload();
                    return None;
                }
                _ => {}
            }
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
                    self.navigate(parent_path(&self.path));
                }
            }
            keys::DELETE => self.delete_selected(),
            keys::F2 => self.begin_rename(),
            keys::ESC => {
                self.confirm_delete = false;
                self.search.clear();
                self.apply_filter();
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

    storage_rows: Vec<(String, String)>,

    storage_updated: u64,
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

            storage_rows: Vec::new(),

            storage_updated: 0,
        }
    }

    fn refresh_storage(&mut self) {
        self.storage_rows.clear();
        self.storage_updated = sys::time_ms();

        let Some(details) = sys::storage_details() else {
            self.storage_rows
                .push(("Storage".to_string(), "Unavailable".to_string()));
            return;
        };

        if details.mounted == 0 {
            self.storage_rows
                .push(("Home volume".to_string(), "Memory only".to_string()));
            self.storage_rows
                .push(("Data disk".to_string(), "Not attached".to_string()));
            self.storage_rows
                .push(("Durability".to_string(), "Lost on shutdown".to_string()));
            return;
        }

        let mib = |bytes: u64| format!("{:.1} MiB", bytes as f64 / 1048576.0);
        let label = core::str::from_utf8(&details.label)
            .unwrap_or("?")
            .trim_end_matches('\0')
            .to_string();

        self.storage_rows
            .push(("Mount".to_string(), "/home/user".to_string()));
        self.storage_rows.push((
            "Filesystem".to_string(),
            if details.filesystem == 1 {
                format!("FAT32 ({})", label)
            } else {
                "Unknown".to_string()
            },
        ));
        self.storage_rows.push((
            "Device".to_string(),
            format!("AHCI disk {}", details.device),
        ));
        self.storage_rows
            .push(("Capacity".to_string(), mib(details.total_bytes)));
        self.storage_rows.push((
            "Used".to_string(),
            mib(details.total_bytes.saturating_sub(details.free_bytes)),
        ));
        self.storage_rows
            .push(("Free".to_string(), mib(details.free_bytes)));
        self.storage_rows.push((
            "Partition".to_string(),
            format!(
                "LBA {} ({} MiB)",
                details.partition_start,
                details.partition_sectors * 512 / (1024 * 1024)
            ),
        ));
        self.storage_rows.push((
            "I/O since boot".to_string(),
            format!(
                "{} KiB read, {} KiB written",
                details.sectors_read / 2,
                details.sectors_written / 2
            ),
        ));
        self.storage_rows.push((
            "Durability".to_string(),
            "Synchronous, no journal".to_string(),
        ));
        self.storage_rows.push((
            "Settings file".to_string(),
            "Versioned v1 with backup".to_string(),
        ));
        if details.healthy == 0 {
            self.storage_rows
                .push(("Health".to_string(), "Metadata unreadable".to_string()));
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

        let sidebar = Rect::new(content.x, content.y, 186, content.h);

        surface.fill_rect(sidebar, palette.sidebar);

        let section_colors = [
            Color::rgb(0, 122, 255),
            Color::rgb(0, 122, 255),
            Color::rgb(255, 45, 85),
            Color::rgb(0, 122, 255),
            Color::rgb(0, 122, 255),
            Color::rgb(142, 142, 147),
            Color::rgb(90, 90, 96),
            Color::rgb(142, 142, 147),
            Color::rgb(0, 122, 255),
            Color::rgb(142, 142, 147),
        ];

        let mut y = content.y + 12;

        for (index, (name, icon)) in SECTIONS.iter().enumerate() {
            let row = Rect::new(sidebar.x + 8, y, sidebar.w - 16, 30);

            if index == self.section {
                surface.fill_rounded(row, 7, Color::rgba(0, 0, 0, 18));
            }

            ui::draw_color_tile(
                surface,
                Rect::new(row.x + 8, row.y + 6, 18, 18),
                section_colors[index],
                *icon,
                theme,
            );

            surface.draw_text(row.x + 32, row.y + 8, name, BODY, palette.text);

            y += 31;
        }

        let body = Rect::new(
            sidebar.right() + 1,
            content.y,
            content.right() - sidebar.right() - 1,
            content.h,
        );

        let mut cy = body.y + 22;

        let x = body.x + 24;

        surface.draw_text(x, cy, self.section_name(), TITLE, palette.text);

        cy += 32;

        let width = (body.w - 48).max(100);

        match self.section {

            0 => {

                surface.draw_text(x, cy, "Light appearance", BODY_MEDIUM, palette.text);

                ui::draw_icon(

                    surface,

                    Icon::Check,

                    Rect::new(x + width - 22, cy + 1, 16, 16),

                    theme.accent_color(),

                    2,

                    theme,

                );

                surface.draw_text(x, cy + 17, "This build ships a single light appearance.", CAPTION, palette.secondary);

                cy += 46;



                surface.draw_text(x, cy, "Accent colour", BODY_MEDIUM, palette.text);

                cy += 26;

                for (index, accent) in theme::ACCENTS.iter().enumerate() {

                    let swatch = Rect::new(x + index as i32 * 30, cy, 24, 24);

                    surface.fill_circle(swatch.x + 12, swatch.y + 12, 10, accent.color());

                    if index == theme.accent_index {

                        surface.stroke_circle(swatch.x + 12, swatch.y + 12, 12, 2, palette.text);

                    }

                }

                cy += 44;



                surface.draw_text(x, cy, "Wallpaper", BODY_MEDIUM, palette.text);

                cy += 22;

                for index in 0..theme::WALLPAPER_COUNT {

                    let tile = Rect::new(x + index as i32 * 100, cy, 90, 54);

                    let (a, b) = wallpaper_swatch(index);

                    surface.vertical_gradient(tile, a, b);

                    surface.stroke_rounded(tile, 8, if index == theme.wallpaper { 2 } else { 1 }, if index == theme.wallpaper { theme.accent_color() } else { palette.hairline });

                    surface.draw_text(tile.x + 7, tile.bottom() - 16, theme::wallpaper_name(index), CAPTION, Color::rgb(255, 255, 255));

                }

                cy += 68;



                surface.draw_text(x, cy, "Animation speed", BODY_MEDIUM, palette.text);

                surface.draw_text(x + width - 40, cy, &format!("{:.2}x", theme.animation_speed), BODY, palette.secondary);

                cy += 22;

                ui::draw_slider(surface, Rect::new(x, cy, width, 20), (theme.animation_speed / 2.0).clamp(0.0, 1.0), theme);

                cy += 38;



                surface.draw_text(x, cy, "Reduce motion", BODY_MEDIUM, palette.text);

                ui::draw_toggle(surface, Rect::new(x + width - 44, cy - 3, 40, 22), theme.reduce_motion, theme);

                cy += 36;



                surface.draw_text(x, cy, "Brightness", BODY_MEDIUM, palette.text);

                cy += 22;

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

                let now = sys::time_ms();

                if self.storage_rows.is_empty()
                    || now.saturating_sub(self.storage_updated) > 900
                {
                    self.refresh_storage();
                }

                for (label, value) in &self.storage_rows {
                    self.info_row(surface, x, &mut cy, width, theme, label, value);
                }

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

                let logo_h = 92;

                let logo_w = (logo_h as f32 * logo_data::COLOR_WIDTH as f32

                    / logo_data::COLOR_HEIGHT as f32) as i32;

                let logo_card = Rect::new(x, cy - 12, 168, 122);

                surface.fill_rounded_gradient(

                    logo_card,

                    22,

                    Color::rgb(250, 252, 255),

                    Color::rgb(232, 240, 253),

                );

                surface.stroke_rounded(logo_card, 22, 1, Color::rgb(212, 224, 243));

                surface.draw_texture(

                    logo_data::COLOR,

                    logo_data::COLOR_WIDTH,

                    logo_data::COLOR_HEIGHT,

                    logo_card.x + (logo_card.w - logo_w) / 2,

                    logo_card.y + (logo_card.h - logo_h) / 2,

                    logo_w,

                    logo_h,

                );

                cy += logo_card.h + 18;

                surface.draw_text(x, cy, "Aster OS 0.3.0", HEADLINE, palette.text);

                cy += 36;

                self.info_row(surface, x, &mut cy, width, theme, "Kernel", "aster 0.3.0 (higher half, x86_64)");

                self.info_row(surface, x, &mut cy, width, theme, "ABI", "Aster ABI 1");

                self.info_row(surface, x, &mut cy, width, theme, "Boot", "Aster Boot, UEFI, GPT + FAT32 ESP");

                self.info_row(surface, x, &mut cy, width, theme, "Desktop", "Aster Desktop (ring 3, software renderer)");

                if let Some(info) = sys::system_info() {

                    self.info_row(surface, x, &mut cy, width, theme, "Uptime", &format!("{} s", info.uptime_ms / 1000));

                    let process_word = if info.process_count == 1 { "process" } else { "processes" };
                    let thread_word = if info.thread_count == 1 { "thread" } else { "threads" };
                    self.info_row(
                        surface,
                        x,
                        &mut cy,
                        width,
                        theme,
                        "Processes",
                        &format!("{} {}, {} {}", info.process_count, process_word, info.thread_count, thread_word),
                    );

                }

                surface.draw_text(x, cy + 8, "A new generation of personal computing.", BODY, theme.accent_color());

            }

        }
    }

    fn row_label(
        &self,
        surface: &mut Surface,
        x: i32,
        y: i32,
        width: i32,
        theme: &Theme,
        title: &str,
        detail: &str,
    ) {
        surface.draw_text(x, y, title, BODY_MEDIUM, theme.palette().text);

        surface.draw_text(x, y + 17, detail, CAPTION, theme.palette().secondary);

        let _ = width;
    }

    fn info_row(
        &self,
        surface: &mut Surface,
        x: i32,
        y: &mut i32,
        width: i32,
        theme: &Theme,
        label: &str,
        value: &str,
    ) {
        let palette = theme.palette();

        surface.draw_text(x, *y, label, BODY, palette.secondary);

        surface.draw_text(x + 190, *y, value, BODY, palette.text);

        surface.fill_rect(Rect::new(x, *y + 24, width, 1), palette.hairline);

        *y += 36;
    }

    fn notice_block(
        &self,
        surface: &mut Surface,
        x: i32,
        cy: i32,
        width: i32,
        theme: &Theme,
        title: &str,
        detail: &str,
    ) {
        let palette = theme.palette();

        surface.fill_rounded(Rect::new(x, cy, width, 96), 12, palette.elevated);

        ui::draw_icon(
            surface,
            Icon::Info,
            Rect::new(x + 16, cy + 18, 18, 18),
            WARNING,
            2,
            theme,
        );

        surface.draw_text(x + 44, cy + 18, title, BODY_MEDIUM, palette.text);

        let lines = wrap_text(surface, detail, BODY, width - 60);

        for (index, line) in lines.iter().take(3).enumerate() {
            surface.draw_text(
                x + 44,
                cy + 40 + index as i32 * 17,
                line,
                BODY,
                palette.secondary,
            );
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect, theme: &Theme) -> Option<Action> {
        let (lx, ly) = local(content, x, y);

        if lx < 186 {
            let index = ((ly - 12) / 31).max(0) as usize;

            if ly >= 12 && index < SECTIONS.len() {
                self.section = index;
            }

            return None;
        }

        let width = (content.w - 186 - 49).max(100);

        let bx = 186 + 25;

        let body_y = ly;

        match self.section {
            0 => {
                if body_y >= 124 && body_y <= 150 {
                    let index = ((lx - bx) / 30).max(0) as usize;

                    if index < theme::ACCENTS.len() {
                        return Some(Action::SetAccent(index));
                    }
                }

                if body_y >= 190 && body_y <= 246 {
                    let index = ((lx - bx) / 100).max(0) as usize;

                    if index < theme::WALLPAPER_COUNT {
                        return Some(Action::SetWallpaper(index));
                    }
                }

                if body_y >= 280 && body_y <= 302 {
                    let value = ((lx - bx) as f32 / width as f32).clamp(0.0, 1.0) * 2.0;

                    return Some(Action::SetAnimationSpeed(value.max(0.25)));
                }

                if body_y >= 315 && body_y <= 340 {
                    return Some(Action::SetReduceMotion(!theme.reduce_motion));
                }

                if body_y >= 368 {
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

fn wallpaper_swatch(style: usize) -> (Color, Color) {
    match style {
        0 => (Color::rgb(238, 226, 210), Color::rgb(134, 124, 120)),

        1 => (Color::rgb(214, 242, 219), Color::rgb(74, 140, 110)),

        2 => (Color::rgb(250, 222, 236), Color::rgb(126, 112, 196)),

        _ => (Color::rgb(220, 238, 250), Color::rgb(84, 136, 184)),
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

        surface.draw_text(
            display.right() - 16 - width,
            display.y + 40,
            &shown,
            DISPLAY,
            color,
        );

        if !self.expression.is_empty() {
            let small = ui::truncate(surface, &self.expression, BODY, display.w - 24);

            surface.draw_text(
                display.x + 14,
                display.y + 14,
                &small,
                BODY,
                palette.secondary,
            );
        }

        let grid = Rect::new(
            content.x + 16,
            display.bottom() + 14,
            content.w - 32,
            content.bottom() - display.bottom() - 30,
        );

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

                let orange = Color::rgb(0xFF, 0x95, 0x00);

                let background = if is_operator {
                    orange
                } else {
                    Color::rgb(0xF2, 0xF2, 0xF7)
                };

                surface.fill_rounded(rect, 9, background);

                let color = if is_operator {
                    Color::rgb(255, 255, 255)
                } else {
                    palette.text
                };

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

                self.display = if self.expression.is_empty() {
                    "0".to_string()
                } else {
                    self.expression.clone()
                };
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

    storage: Option<StorageDetails>,

    logs: [u8; 3072],

    log_len: usize,

    view_logs: bool,

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

            storage: None,

            logs: [0; 3072],

            log_len: 0,

            view_logs: false,

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

        self.storage = sys::storage_details();

        let logged = sys::log_read(&mut self.logs);

        self.log_len = if logged > 0 { logged as usize } else { 0 };
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
            (
                "Uptime",
                info.as_ref()
                    .map(|i| format!("{} s", i.uptime_ms / 1000))
                    .unwrap_or("-".into()),
            ),
            (
                "Processes",
                info.as_ref()
                    .map(|i| format!("{}", i.process_count))
                    .unwrap_or("-".into()),
            ),
            (
                "Threads",
                info.as_ref()
                    .map(|i| format!("{}", i.thread_count))
                    .unwrap_or("-".into()),
            ),
            (
                "Switches",
                info.as_ref()
                    .map(|i| format!("{}", i.context_switches))
                    .unwrap_or("-".into()),
            ),
            (
                "Storage",
                self.storage
                    .map(|details| {
                        if details.mounted != 0 && details.total_bytes > 0 {
                            format!(
                                "{}%",
                                details
                                    .total_bytes
                                    .saturating_sub(details.free_bytes)
                                    .saturating_mul(100)
                                    / details.total_bytes
                            )
                        } else {
                            "-".to_string()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string()),
            ),
            (
                "Disk free",
                self.storage
                    .map(|details| {
                        if details.mounted != 0 {
                            format!("{} MiB", details.free_bytes / (1024 * 1024))
                        } else {
                            "-".to_string()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ];

        let card_w = (content.w - 40 - 5 * 12) / 6;

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

            surface.fill_rect(
                Rect::new(plot.x, plot.bottom() - 1, plot.w, 1),
                palette.hairline,
            );

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

        surface.draw_text(
            table.x + 12,
            table.y + 10,
            "Processes",
            BODY_MEDIUM,
            if self.view_logs {
                palette.secondary
            } else {
                palette.text
            },
        );

        surface.draw_text(
            table.x + 112,
            table.y + 10,
            "Logs",
            BODY_MEDIUM,
            if self.view_logs {
                palette.text
            } else {
                palette.secondary
            },
        );

        if self.view_logs {
            let mut starts = [0usize; 48];
            let mut ends = [0usize; 48];
            let mut lines = 0usize;
            let mut line_start = 0usize;
            for (index, byte) in self.logs[..self.log_len].iter().enumerate() {
                if *byte == b'\n' {
                    if lines < starts.len() {
                        starts[lines] = line_start;
                        ends[lines] = index;
                        lines += 1;
                    }
                    line_start = index + 1;
                }
            }
            if line_start < self.log_len && lines < starts.len() {
                starts[lines] = line_start;
                ends[lines] = self.log_len;
                lines += 1;
            }
            let first = lines.saturating_sub(16);
            let mut line_y = table.y + 36;
            for index in first..lines {
                if line_y > table.bottom() - 18 {
                    break;
                }
                let bytes = &self.logs[starts[index]..ends[index]];
                if let Ok(text) = core::str::from_utf8(bytes) {
                    if !text.is_empty() {
                        ui::draw_text_truncated(
                            surface,
                            table.x + 12,
                            line_y,
                            text,
                            MONO,
                            palette.secondary,
                            table.w - 24,
                        );
                    }
                }
                line_y += 16;
            }
            return;
        }

        let mut row_y = table.y + 36;

        surface.draw_text(table.x + 12, row_y, "PID", CAPTION, palette.tertiary);

        surface.draw_text(table.x + 80, row_y, "THREADS", CAPTION, palette.tertiary);

        surface.draw_text(table.x + 170, row_y, "NAME", CAPTION, palette.tertiary);

        row_y += 18;

        for process in &self.processes {
            if row_y > table.bottom() - 20 {
                break;
            }

            let name = core::str::from_utf8(&process.name)
                .unwrap_or("?")
                .trim_end_matches('\0');

            surface.draw_text(
                table.x + 12,
                row_y,
                &format!("{}", process.pid),
                BODY,
                palette.text,
            );

            surface.draw_text(
                table.x + 80,
                row_y,
                &format!("{}", process.threads),
                BODY,
                palette.text,
            );

            ui::draw_text_truncated(
                surface,
                table.x + 170,
                row_y,
                name,
                BODY,
                palette.text,
                table.w - 200,
            );

            surface.fill_circle(
                table.right() - 24,
                row_y + 7,
                3,
                if process.state == 0 { SUCCESS } else { WARNING },
            );

            row_y += 24;
        }
    }

    fn mouse(&mut self, x: i32, y: i32, content: Rect) -> Option<Action> {
        let (mx, my) = local(content, x, y);
        // The table begins after the title, cards and the two graphs.
        let table_y = 18 + 30 + 80 + 130 + 16;
        if my >= table_y + 4 && my <= table_y + 30 {
            if mx >= 12 && mx <= 104 {
                self.view_logs = false;
            } else if mx >= 112 && mx <= 160 {
                self.view_logs = true;
            }
        }
        None
    }
}

// ===========================================================================

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
            path: "/home/user/DOCS/UNTITLED.TXT".to_string(),

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
        self.path
            .rsplit('/')
            .next()
            .unwrap_or("untitled")
            .to_string()
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

        surface.fill_rect(
            Rect::new(toolbar.x, toolbar.bottom() - 1, toolbar.w, 1),
            palette.hairline,
        );

        let save = Rect::new(toolbar.right() - 92, toolbar.y + 9, 80, 24);

        ui::draw_button(surface, save, "Save", true, theme, false, true);

        let new_button = Rect::new(toolbar.right() - 176, toolbar.y + 9, 76, 24);

        ui::draw_button(surface, new_button, "New", false, theme, false, true);

        let name = ui::truncate(
            surface,
            &format!(
                "{}{}",
                self.display_name(),
                if self.dirty { " *" } else { "" }
            ),
            BODY_MEDIUM,
            toolbar.w - 200,
        );

        surface.draw_text(
            toolbar.x + 14,
            toolbar.y + 13,
            &name,
            BODY_MEDIUM,
            palette.text,
        );

        let editor = Rect::new(
            content.x,
            toolbar.bottom(),
            content.w,
            content.bottom() - toolbar.bottom() - 22,
        );

        let line_height = 18;

        let lines: Vec<&str> = self.text.split('\n').collect();

        let visible = (editor.h / line_height).max(1);

        let cursor_line = self.text[..self.cursor.min(self.text.len())]
            .matches('\n')
            .count() as i32;

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

        surface.fill_rect(
            Rect::new(content.x, content.bottom() - 22, content.w, 1),
            palette.hairline,
        );

        surface.draw_text(
            content.x + 12,
            content.bottom() - 18,
            &self.status,
            CAPTION,
            palette.secondary,
        );
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
                self.cursor = self.text[self.cursor..]
                    .find('\n')
                    .map(|i| self.cursor + i)
                    .unwrap_or(self.text.len());
            }

            code if code == b's' as u32 && ctrl => return Some(self.save()),

            code if code == b'n' as u32 && ctrl => {
                self.path = "/home/user/DOCS/UNTITLED.TXT".to_string();

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
                self.path = "/home/user/DOCS/UNTITLED.TXT".to_string();

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
            let path = "/home/user/NOTES/FIRST-NOTE.TXT".to_string();

            let _ = sys::write_file(
                &path,
                b"Welcome to Notes.\n\nEverything you type is saved to the ramfs.\n",
            );

            app.reload();
        }

        app.load_selected();

        app
    }

    fn reload(&mut self) {
        self.notes.clear();

        if let Some(entries) = sys::read_dir("/home/user/NOTES") {
            let mut names: Vec<(String, bool)> = entries
                .into_iter()
                .map(|(name, is_dir, _)| (name, is_dir))
                .collect();

            names.sort();

            for (name, is_dir) in names {
                if !is_dir {
                    let path = format!("/home/user/NOTES/{}", name);

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
        // Persistent note names must not collide across reboots, and the
        // volume uses 8.3 names: NOTE-1.TXT, NOTE-2.TXT, ...
        let mut index = 1u32;

        let (name, path) = loop {
            let candidate = format!("NOTE-{}.TXT", index);

            let path = format!("/home/user/NOTES/{}", candidate);

            if sys::stat(&path).is_none() || index >= 999 {
                break (candidate, path);
            }

            index += 1;
        };

        if sys::write_file(&path, b"New note\n") {
            self.reload();

            if let Some(index) = self.notes.iter().position(|(n, _)| *n == name) {
                self.selected = index;
            }

            self.load_selected();

            self.buffer = String::new();

            self.dirty = true;

            sys::log(format!("notes: created {}", name).as_bytes());
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

        ui::draw_icon(
            surface,
            Icon::Plus,
            add.inset(6),
            theme.text_on_accent(),
            1,
            theme,
        );

        let mut y = sidebar.y + 52;

        for (index, (name, _)) in self.notes.iter().enumerate() {
            if y > sidebar.bottom() - 30 {
                break;
            }

            let row = Rect::new(sidebar.x + 8, y, sidebar.w - 16, 30);

            if index == self.selected {
                surface.fill_rounded(row, 7, theme.accent_color().with_alpha(40));
            }

            surface.draw_text(
                row.x + 10,
                row.y + 8,
                &ui::truncate(surface, name, BODY, row.w - 20),
                BODY,
                palette.text,
            );

            y += 32;
        }

        let editor = Rect::new(
            sidebar.right(),
            content.y,
            content.right() - sidebar.right(),
            content.h,
        );

        let toolbar = Rect::new(editor.x, editor.y, editor.w, 40);

        surface.fill_rect(toolbar, palette.surface);

        surface.fill_rect(
            Rect::new(toolbar.x, toolbar.bottom() - 1, toolbar.w, 1),
            palette.hairline,
        );

        let save = Rect::new(toolbar.right() - 88, toolbar.y + 8, 76, 24);

        ui::draw_button(surface, save, "Save", true, theme, false, true);

        let delete = Rect::new(toolbar.right() - 168, toolbar.y + 8, 70, 24);

        ui::draw_button(surface, delete, "Delete", false, theme, false, true);

        let text_area = Rect::new(
            editor.x + 20,
            toolbar.bottom() + 16,
            editor.w - 40,
            editor.bottom() - toolbar.bottom() - 40,
        );

        let mut y = text_area.y;

        for (index, line) in wrap_text(surface, &self.buffer, BODY, text_area.w)
            .iter()
            .take(((text_area.h / 18).max(1)) as usize)
            .enumerate()
        {
            surface.draw_text(text_area.x, y, line, BODY, palette.text);

            y += 18;

            let _ = index;
        }

        if (sys::time_ms() / 500) % 2 == 0 && self.dirty {
            surface.fill_rect(
                Rect::new(text_area.x + 2, text_area.bottom() - 4, 80, 2),
                theme.accent_color(),
            );
        }

        surface.draw_text(
            editor.x + 20,
            editor.bottom() - 22,
            &self.status,
            CAPTION,
            palette.secondary,
        );
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

        surface.draw_text(
            header.x,
            header.y,
            &format!("{} {}", month_name(self.month), self.year),
            HEADLINE,
            palette.text,
        );

        let prev = Rect::new(header.right() - 84, header.y + 2, 32, 26);

        let next = Rect::new(header.right() - 44, header.y + 2, 32, 26);

        for (rect, icon) in [(prev, Icon::ChevronLeft), (next, Icon::ChevronRight)] {
            surface.fill_rounded(rect, 7, palette.elevated);

            ui::draw_icon(surface, icon, rect.inset(8), palette.secondary, 1, theme);
        }

        let grid = Rect::new(
            content.x + 20,
            header.bottom() + 18,
            content.w - 40,
            content.bottom() - header.bottom() - 50,
        );

        let cell_w = grid.w / 7;

        let cell_h = (grid.h / 7).max(30);

        for (index, label) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .enumerate()
        {
            surface.draw_text(
                grid.x + index as i32 * cell_w + 8,
                grid.y,
                label,
                CAPTION,
                palette.tertiary,
            );
        }

        let start_weekday = weekday_of(self.year, self.month, 1);

        let days = days_in_month(self.year, self.month);

        for day in 1..=days {
            let cell_index = start_weekday + day as i32 - 1;

            let col = cell_index % 7;

            let row = cell_index / 7;

            let cell = Rect::new(
                grid.x + col * cell_w + 4,
                grid.y + 22 + row * cell_h + 4,
                cell_w - 8,
                cell_h - 8,
            );

            let is_today = (self.year, self.month, day) == self.today;

            if is_today {
                surface.fill_rounded(cell, 9, theme.accent_color());
            } else if day == self.day {
                surface.stroke_rounded(cell, 9, 1, palette.hairline);
            }

            let color = if is_today {
                theme.text_on_accent()
            } else {
                palette.text
            };

            surface.draw_text(cell.x + 9, cell.y + 7, &format!("{}", day), BODY, color);
        }

        if let Some(now) = sys::rtc() {
            surface.draw_text(
                content.x + 20,
                content.bottom() - 30,
                &format!(
                    "Today is {} {:02}, {} - {:02}:{:02}:{:02}",
                    month_name(now.month),
                    now.day,
                    now.year,
                    now.hour,
                    now.minute,
                    now.second
                ),
                BODY,
                palette.secondary,
            );
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
}

impl PhotosApp {
    fn new(theme: &Theme) -> Self {
        let mut app = Self {
            selected: theme.wallpaper,

            thumbs: Vec::new(),
        };

        app.build();

        app
    }

    fn build(&mut self) {
        self.thumbs.clear();

        let width = 288;

        let height = 120;

        for index in 0..theme::WALLPAPER_COUNT {
            let mut pixels = vec![0u32; (width * height) as usize];

            theme::paint_wallpaper(&mut pixels, width, height, index);

            for pixel in &mut pixels {
                *pixel |= 0xFF00_0000;
            }

            self.thumbs.push((index, pixels, width, height));
        }
    }

    fn tick(&mut self, _theme: &Theme) {}

    fn draw(&mut self, surface: &mut Surface, content: Rect, theme: &Theme) {
        let palette = theme.palette();

        surface.fill_rect(content, palette.surface);

        surface.draw_text(
            content.x + 20,
            content.y + 18,
            "Wallpapers",
            TITLE,
            palette.text,
        );

        surface.draw_text(
            content.x + 20,
            content.y + 42,
            "Generated procedural wallpapers. Image file decoding is not implemented.",
            CAPTION,
            palette.secondary,
        );

        let grid_x = content.x + 20;

        let grid_y = content.y + 70;

        let card_w = (content.w - 52) / 2;

        let card_h = 180;

        for (slot, (index, pixels, width, height)) in self.thumbs.iter().enumerate() {
            let col = slot as i32 % 2;

            let row = slot as i32 / 2;

            let card = Rect::new(
                grid_x + col * (card_w + 12),
                grid_y + row * (card_h + 12),
                card_w,
                card_h,
            );

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

            surface.draw_text(
                card.x + 12,
                card.bottom() - 30,
                theme::wallpaper_name(*index),
                BODY_MEDIUM,
                palette.text,
            );

            if *index == theme.wallpaper {
                surface.draw_text(
                    card.right() - 58,
                    card.bottom() - 30,
                    "Active",
                    CAPTION,
                    theme.accent_color(),
                );
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
