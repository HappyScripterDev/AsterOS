//! Aster Desktop - ring-3 desktop environment entry point.

#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod apps;
mod desktop;
mod font_data;
mod gfx;
mod icons_data;
mod logo_data;
mod sys;
mod theme;
mod ui;
mod wm;

use alloc::format;
use core::arch::global_asm;
use core::panic::PanicInfo;

global_asm!(
    r#"
    .section .text.entry, "ax"
    .global _start
_start:
    cld
    xor rbp, rbp
    call desktop_main
1:
    hlt
    jmp 1b
"#
);

#[no_mangle]
pub extern "C" fn desktop_main() -> ! {
    // 48 MiB bump heap: backbuffer, wallpaper, blur buffer, shadows and UI state.
    if !allocator::init(48 * 1024 * 1024) {
        sys::log(b"desktop: heap initialisation failed");
    }
    if let Some(info) = sys::system_info() {
        sys::log(
            format!(
                "desktop: heap ready, {} MiB free, {} threads",
                info.free_memory_kib / 1024,
                info.thread_count
            )
            .as_bytes(),
        );
    }
    sys::log(b"desktop: entered ring 3");
    desktop::run();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut message = [0u8; 160];
    let mut length = 0;
    for byte in format_args_panic(info).bytes() {
        if length < message.len() {
            message[length] = byte;
            length += 1;
        }
    }
    sys::log(&message[..length]);
    loop {
        sys::sleep_ms(1000);
    }
}

fn format_args_panic(info: &PanicInfo) -> alloc::string::String {
    alloc::format!("desktop: PANIC: {}", info)
}
