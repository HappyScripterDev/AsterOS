//! Aster configuration service.
//!
//! Owns `/home/user/SETTINGS.CNF`. The desktop sends the serialized v1 settings
//! blob over the bootstrap channel; configd validates it, writes a temporary
//! file, flushes, keeps a backup and renames the temporary file into place.
//! A failed service does not take the desktop down: the desktop falls back to
//! direct file I/O and the supervisor restarts configd.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

const SYS_EXIT: u64 = 1;
const SYS_LOG: u64 = 11;
const SYS_OPEN: u64 = 12;
const SYS_READ: u64 = 13;
const SYS_WRITE_FD: u64 = 14;
const SYS_CLOSE: u64 = 15;
const SYS_STAT: u64 = 18;
const SYS_UNLINK: u64 = 19;
const SYS_RENAME: u64 = 34;
const SYS_SYNC: u64 = 36;
const SYS_CHANNEL_SEND: u64 = 25;
const SYS_CHANNEL_RECV: u64 = 26;

const HANDLE: u32 = 3;
const MAX_SETTINGS: usize = 240;
const PATH: &[u8] = b"/home/user/SETTINGS.CNF\0";
const TEMP: &[u8] = b"/home/user/SETTINGS.NEW\0";
const BACKUP: &[u8] = b"/home/user/SETTINGS.BAK\0";
const BAD: &[u8] = b"/home/user/SETTINGS.BAD\0";

const OPEN_READ: u32 = 1;
const OPEN_WRITE: u32 = 2;
const OPEN_CREATE: u32 = 4;
const OPEN_TRUNC: u32 = 8;

global_asm!(
    r#"
    .section .text.entry, "ax"
    .global _start
_start:
    cld
    xor rbp, rbp
    call service_main
1:
    hlt
    jmp 1b
"#
);

#[inline]
unsafe fn syscall(number: u64, a: u64, b: u64, c: u64) -> i64 {
    let result: i64;
    asm!(
        "int 0x80",
        inlateout("rax") number as i64 => result,
        in("rdi") a,
        in("rsi") b,
        in("rdx") c,
        in("r10") 0u64,
        in("r8") 0u64,
        in("r9") 0u64,
        options(nostack)
    );
    result
}

fn log(message: &[u8]) {
    unsafe {
        syscall(SYS_LOG, message.as_ptr() as u64, message.len() as u64, 0);
    }
}

fn exists(path: &[u8]) -> bool {
    let mut stat = [0u8; 24];
    unsafe { syscall(SYS_STAT, path.as_ptr() as u64, stat.as_mut_ptr() as u64, 0) >= 0 }
}

fn read_settings(out: &mut [u8]) -> usize {
    let fd = unsafe { syscall(SYS_OPEN, PATH.as_ptr() as u64, OPEN_READ as u64, 0) };
    if fd < 0 {
        return 0;
    }
    let count = unsafe {
        syscall(
            SYS_READ,
            fd as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    };
    unsafe {
        syscall(SYS_CLOSE, fd as u64, 0, 0);
    }
    if count > 0 {
        count as usize
    } else {
        0
    }
}

fn write_file(path: &[u8], data: &[u8]) -> bool {
    let flags = OPEN_WRITE | OPEN_CREATE | OPEN_TRUNC;
    let fd = unsafe { syscall(SYS_OPEN, path.as_ptr() as u64, flags as u64, 0) };
    if fd < 0 {
        return false;
    }
    let mut offset = 0usize;
    let mut ok = true;
    while offset < data.len() {
        let written = unsafe {
            syscall(
                SYS_WRITE_FD,
                fd as u64,
                data[offset..].as_ptr() as u64,
                (data.len() - offset) as u64,
            )
        };
        if written <= 0 {
            ok = false;
            break;
        }
        offset += written as usize;
    }
    unsafe {
        syscall(SYS_CLOSE, fd as u64, 0, 0);
    }
    ok
}

fn sync() {
    unsafe {
        syscall(SYS_SYNC, 0, 0, 0);
    }
}

fn save(data: &[u8]) -> bool {
    if !write_file(TEMP, data) {
        return false;
    }
    sync();
    if exists(PATH) {
        let _ = unsafe { syscall(SYS_UNLINK, BACKUP.as_ptr() as u64, 0, 0) };
        let _ = unsafe { syscall(SYS_RENAME, PATH.as_ptr() as u64, BACKUP.as_ptr() as u64, 0) };
    }
    let renamed =
        unsafe { syscall(SYS_RENAME, TEMP.as_ptr() as u64, PATH.as_ptr() as u64, 0) } >= 0;
    let stored = if renamed {
        true
    } else {
        let direct = write_file(PATH, data);
        let _ = unsafe { syscall(SYS_UNLINK, TEMP.as_ptr() as u64, 0, 0) };
        direct
    };
    sync();
    stored
}

fn valid_settings(data: &[u8]) -> bool {
    if data.is_empty() || data.len() > MAX_SETTINGS {
        return false;
    }
    if !data.starts_with(b"version=1\n") {
        return false;
    }
    for byte in data {
        if *byte != b'\n' && (*byte < 32 || *byte > 126) {
            return false;
        }
    }
    core::str::from_utf8(data).is_ok()
}

/// A 0.2 file had no version line. It is still readable and the desktop can
/// migrate it; do not destroy it as if it were corrupt.
fn legacy_settings(data: &[u8]) -> bool {
    if data.is_empty() || data.len() > MAX_SETTINGS {
        return false;
    }
    for byte in data {
        if *byte != b'\n' && (*byte < 32 || *byte > 126) {
            return false;
        }
    }
    if core::str::from_utf8(data).is_err() {
        return false;
    }
    data.windows(7).any(|window| window == b"accent=")
        || data.windows(10).any(|window| window == b"wallpaper=")
        || data.windows(11).any(|window| window == b"brightness=")
}

fn acceptable_settings(data: &[u8]) -> bool {
    valid_settings(data) || legacy_settings(data)
}

fn send(data: &[u8]) -> bool {
    unsafe {
        syscall(
            SYS_CHANNEL_SEND,
            HANDLE as u64,
            data.as_ptr() as u64,
            data.len() as u64,
        ) == data.len() as i64
    }
}

#[no_mangle]
pub extern "C" fn service_main() -> ! {
    let mut settings = [0u8; MAX_SETTINGS];
    let mut length = read_settings(&mut settings);

    if length > 0 && !acceptable_settings(&settings[..length]) {
        log(b"configd: corrupt settings preserved as SETTINGS.BAD\n");
        let _ = unsafe { syscall(SYS_UNLINK, BAD.as_ptr() as u64, 0, 0) };
        let _ = unsafe { syscall(SYS_RENAME, PATH.as_ptr() as u64, BAD.as_ptr() as u64, 0) };
        length = 0;
    }

    if length > 0 && !valid_settings(&settings[..length]) {
        log(b"configd: legacy settings loaded, desktop will migrate them\n");
    }

    if length == 0 {
        let defaults = b"version=1\naccent=blue\nanimations=1.00\nwallpaper=0\nreduce_motion=0\nbrightness=1.00\ngfx=1,1,1,1,60,1.00\nmotion=1,1,1,1,1\ndock=0,1.00,0.45,0,1\n";
        if save(defaults) {
            settings[..defaults.len()].copy_from_slice(defaults);
            length = defaults.len();
            log(b"configd: wrote default settings\n");
        } else {
            log(b"configd: could not write default settings\n");
        }
    }

    log(b"configd: started, serving settings\n");

    loop {
        let mut message = [0u8; 256];
        let received = unsafe {
            syscall(
                SYS_CHANNEL_RECV,
                HANDLE as u64,
                message.as_mut_ptr() as u64,
                message.len() as u64,
            )
        };
        if received <= 0 {
            log(b"configd: channel closed, exiting\n");
            unsafe { syscall(SYS_EXIT, 0, 0, 0) };
        }
        let message = &message[..received as usize];

        if message == b"PING" {
            if !send(b"PONG") {
                unsafe { syscall(SYS_EXIT, 1, 0, 0) };
            }
        } else if message == b"GET" {
            let reply = if length > 0 {
                &settings[..length]
            } else {
                b"version=1\naccent=blue\nanimations=1.00\nwallpaper=0\nreduce_motion=0\nbrightness=1.00\ngfx=1,1,1,1,60,1.00\nmotion=1,1,1,1,1\ndock=0,1.00,0.45,0,1\n"
            };
            if !send(reply) {
                unsafe { syscall(SYS_EXIT, 2, 0, 0) };
            }
        } else if message.starts_with(b"SET\n") {
            let data = &message[4..];
            if valid_settings(data) && save(data) {
                settings[..data.len()].copy_from_slice(data);
                length = data.len();
                log(b"configd: settings saved\n");
                if !send(b"OK") {
                    unsafe { syscall(SYS_EXIT, 3, 0, 0) };
                }
            } else {
                log(b"configd: rejected settings\n");
                if !send(b"ERR") {
                    unsafe { syscall(SYS_EXIT, 4, 0, 0) };
                }
            }
        } else if message == b"EXIT" {
            log(b"configd: exit requested\n");
            let _ = send(b"BYE");
            unsafe { syscall(SYS_EXIT, 0, 0, 0) };
        } else {
            let _ = send(b"ERR");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        syscall(SYS_EXIT, 99, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}
