//! Fault containment probe.
//!
//! Deliberately writes to a supervisor-only kernel address. The kernel must
//! terminate this process with a fault status while the desktop survives.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

const SYS_EXIT: u64 = 1;
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
    log(b"faultprobe: writing to a kernel address\n");
    unsafe {
        (0xFFFF_FFFF_8000_0000u64 as *mut u64).write_volatile(0xDEAD_BEEF);
    }
    log(b"faultprobe: unexpectedly survived the write\n");
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
