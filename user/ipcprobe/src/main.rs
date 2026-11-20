//! IPC probe.
//!
//! The parent passes one channel endpoint as a bootstrap handle. By convention
//! it is installed at handle 3 in the child's table. The child blocks in
//! CHANNEL_RECV until the parent sends "ping", replies "pong", and exits 0.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

const SYS_EXIT: u64 = 1;
const SYS_LOG: u64 = 11;
const SYS_CHANNEL_SEND: u64 = 25;
const SYS_CHANNEL_RECV: u64 = 26;
const BOOTSTRAP_HANDLE: u32 = 3;

global_asm!(
    r#"
    .section .text.entry, "ax"
    .global _start
_start:
    cld
    xor rbp, rbp
    call probe_main
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
        options(nostack)
    );
    result
}

fn log(message: &[u8]) {
    unsafe {
        syscall(SYS_LOG, message.as_ptr() as u64, message.len() as u64, 0);
    }
}

#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    log(b"ipcprobe: waiting on bootstrap handle 3\n");

    let mut buffer = [0u8; 64];
    let received = unsafe {
        syscall(
            SYS_CHANNEL_RECV,
            BOOTSTRAP_HANDLE as u64,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
        )
    };
    if received <= 0 {
        log(b"ipcprobe: receive failed\n");
        unsafe { syscall(SYS_EXIT, 45, 0, 0) };
    }

    if &buffer[..received as usize] != b"ping" {
        log(b"ipcprobe: unexpected message\n");
        unsafe { syscall(SYS_EXIT, 46, 0, 0) };
    }

    log(b"ipcprobe: received ping, sending pong\n");
    let sent = unsafe {
        syscall(
            SYS_CHANNEL_SEND,
            BOOTSTRAP_HANDLE as u64,
            b"pong".as_ptr() as u64,
            4,
        )
    };
    if sent != 4 {
        log(b"ipcprobe: send failed\n");
        unsafe { syscall(SYS_EXIT, 47, 0, 0) };
    }

    unsafe { syscall(SYS_EXIT, 0, 0, 0) };
    loop {
        core::hint::spin_loop();
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
