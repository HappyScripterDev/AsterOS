//! Desktop-side service supervisor.
//!
//! Currently one service: `configd`, which owns the persistent settings file.
//! The desktop starts it with a bootstrap channel, talks to it with a short
//! timeout, detects a crash through `PROC_LIST`, reaps it and restarts it.
//! When the service is unavailable, settings fall back to direct file I/O so a
//! service failure cannot lock the user out.

use crate::sys;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Stopped,
    Running,
    Failed,
}

pub struct Manager {
    pid: u64,
    handle: u32,
    restarts: u32,
    state: State,
    last_error: i64,
    next_check: u64,
    retry: bool,
}

impl Manager {
    pub const fn new() -> Self {
        Self {
            pid: 0,
            handle: 0,
            restarts: 0,
            state: State::Stopped,
            last_error: 0,
            next_check: 0,
            retry: false,
        }
    }

    pub fn start(&mut self) -> bool {
        if self.pid != 0 {
            return self.state == State::Running;
        }
        let Some((first, second)) = sys::channel_create() else {
            self.state = State::Failed;
            self.last_error = -1;
            sys::log(b"service: cannot create the configd channel\n");
            return false;
        };

        let pid = sys::spawn_with_handle("/home/user/configd.elf", second);
        let _ = sys::channel_close(second);
        if pid <= 0 {
            let _ = sys::channel_close(first);
            self.state = State::Failed;
            self.last_error = pid;
            self.retry = false;
            sys::log(b"service: configd is unavailable\n");
            return false;
        }

        self.pid = pid as u64;
        self.handle = first;
        self.state = State::Running;

        let mut reply = [0u8; 16];
        let received = self.request(b"PING", &mut reply, 700);
        if received == 4 && &reply[..4] == b"PONG" {
            sys::log(b"service: configd running\n");
            self.retry = true;
            true
        } else {
            self.last_error = received;
            self.state = State::Failed;
            let _ = sys::channel_close(self.handle);
            self.handle = 0;
            self.pid = 0;
            sys::log(b"service: configd did not answer PING\n");
            false
        }
    }

    fn request(&mut self, message: &[u8], reply: &mut [u8], timeout_ms: u64) -> i64 {
        if self.handle == 0 || self.state != State::Running {
            return -1;
        }
        let sent = sys::channel_send(self.handle, message);
        if sent != message.len() as i64 {
            self.last_error = sent;
            sys::log(b"service: configd send failed\n");
            return sent;
        }
        let received = sys::channel_recv_timeout(self.handle, reply, timeout_ms);
        if received <= 0 {
            self.last_error = received;
            sys::log(b"service: configd reply failed\n");
        }
        received
    }

    pub fn get_settings(&mut self, out: &mut [u8]) -> Option<usize> {
        let received = self.request(b"GET", out, 700);
        if received > 0 {
            Some(received as usize)
        } else {
            None
        }
    }

    pub fn set_settings(&mut self, content: &str) -> bool {
        let bytes = content.as_bytes();
        if bytes.len() + 4 > 240 {
            return false;
        }
        let mut message = [0u8; 240];
        message[..4].copy_from_slice(b"SET\n");
        message[4..4 + bytes.len()].copy_from_slice(bytes);
        let mut reply = [0u8; 8];
        let received = self.request(&message[..4 + bytes.len()], &mut reply, 900);
        received == 2 && &reply[..2] == b"OK"
    }

    pub fn tick(&mut self) {
        let now = sys::time_ms();
        if now < self.next_check {
            return;
        }
        self.next_check = now + 2000;

        if self.pid == 0 {
            return;
        }

        let mut infos: [aster_abi::ProcessInfo; 16] = unsafe { core::mem::zeroed() };
        let count = sys::process_list(&mut infos);
        let mut found = None;
        for process in &infos[..count] {
            if process.pid == self.pid {
                found = Some(process.state);
            }
        }

        match found {
            None => {
                sys::log(b"service: configd disappeared, restarting\n");
                self.reap_and_restart();
            }
            Some(1) => {
                sys::log(b"service: configd exited, restarting\n");
                self.reap_and_restart();
            }
            Some(_) => {
                self.state = State::Running;
            }
        }
    }

    fn reap_and_restart(&mut self) {
        let _ = sys::channel_close(self.handle);
        self.handle = 0;
        let mut status = 0i64;
        let _ = sys::wait(self.pid, &mut status);
        self.pid = 0;
        self.restarts = self.restarts.saturating_add(1);
        self.state = State::Stopped;
        self.next_check = sys::time_ms() + 2000;
        self.start();
    }

    pub fn restart(&mut self) {
        sys::log(b"service: restart requested\n");
        if self.pid != 0 {
            let mut reply = [0u8; 8];
            let _ = self.request(b"EXIT", &mut reply, 700);
            let _ = sys::channel_close(self.handle);
            self.handle = 0;
            let mut status = 0i64;
            let _ = sys::wait(self.pid, &mut status);
            self.pid = 0;
        }
        self.restarts = self.restarts.saturating_add(1);
        self.state = State::Stopped;
        self.start();
    }
}
