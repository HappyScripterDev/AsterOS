//! Process isolation probe.
//!
//! Allocates one private page, writes and verifies a marker, logs a line, and
//! exits with status 42. The parent can only observe the status; the marker
//! lives in this process's address space.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

const SYS_EXIT: u64 = 1;
const SYS_VM_ALLOC: u64 = 10;
const SYS_LOG: u64 = 11;

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
    log(b"isoprobe: running with a private address space\n");

    let page = unsafe { syscall(SYS_VM_ALLOC, 4096, 0, 0) };
    if page <= 0 {
        log(b"isoprobe: VM_ALLOC failed\n");
        unsafe { syscall(SYS_EXIT, 43, 0, 0) };
    }

    let marker = (page as u64) as *mut u64;
    unsafe {
        marker.write_volatile(0xA57E_0001);
        if marker.read_volatile() != 0xA57E_0001 {
            log(b"isoprobe: private page verification failed\n");
            syscall(SYS_EXIT, 44, 0, 0);
        }
    }

    log(b"isoprobe: private page verified, exiting with 42\n");
    unsafe { syscall(SYS_EXIT, 42, 0, 0) };
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
