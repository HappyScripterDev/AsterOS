//! Kernel logging: serial console plus an in-memory ring buffer.
//!
//! The ring buffer backs the desktop's log viewer and `dmesg`-style
//! diagnostics without requiring a filesystem.

use core::fmt::{self, Write};

use crate::sync::IrqLock;

pub const LOG_RING_SIZE: usize = 64 * 1024;

struct Ring {
    data: [u8; LOG_RING_SIZE],
    head: usize,
    total: u64,
}

impl Ring {
    const fn new() -> Self {
        Self {
            data: [0; LOG_RING_SIZE],
            head: 0,
            total: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        self.data[self.head] = byte;
        self.head = (self.head + 1) % LOG_RING_SIZE;
        self.total += 1;
    }

    fn copy_out(&self, out: &mut [u8]) -> usize {
        let available = self.total.min(LOG_RING_SIZE as u64) as usize;
        let count = available.min(out.len());
        let start = (self.head + LOG_RING_SIZE - count) % LOG_RING_SIZE;
        for i in 0..count {
            out[i] = self.data[(start + i) % LOG_RING_SIZE];
        }
        count
    }
}

static RING: IrqLock<Ring> = IrqLock::new(Ring::new());
static SERIAL_LOCK: IrqLock<()> = IrqLock::new(());

pub fn init() {
    crate::arch::serial::init();
}

pub fn write_raw(bytes: &[u8]) {
    let _guard = SERIAL_LOCK.lock();
    for &byte in bytes {
        if byte == b'\n' {
            crate::arch::serial::put(b'\r');
        }
        crate::arch::serial::put(byte);
    }
    let mut ring = RING.lock();
    for &byte in bytes {
        ring.push(byte);
    }
}

struct Writer;

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_raw(s.as_bytes());
        Ok(())
    }
}

pub fn log(args: fmt::Arguments) {
    let mut writer = Writer;
    let _ = writer.write_fmt(args);
}

/// Timestamped kernel message, e.g. `[   12.345] mm: ...`.
pub fn log_line(level: &str, args: fmt::Arguments) {
    let ms = crate::time::uptime_ms();
    let _ = write!(
        Writer,
        "[{:>6}.{:03}] {}: ",
        ms / 1000,
        ms % 1000,
        level
    );
    let _ = Writer.write_fmt(args);
    let _ = Writer.write_str("\n");
}

pub fn read_log(out: &mut [u8]) -> usize {
    RING.lock().copy_out(out)
}

pub fn log_bytes() -> u64 {
    RING.lock().total
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::logging::log_line("info", format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::logging::log_line("warn", format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::logging::log_line("error", format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => { $crate::logging::log_line("debug", format_args!($($arg)*)) };
}
