//! Aster Kernel entry point and boot sequence.
//!
//! Ordering is deliberate:
//!   1. diagnostics (serial),
//!   2. boot contract validation,
//!   3. CPU tables (GDT/TSS, IDT, PIC, PIT),
//!   4. physical frames, virtual memory, kernel heap,
//!   5. self-tests,
//!   6. scheduler and drivers,
//!   7. interrupts on, idle.
//!
//! Nothing here is decorative: each stage either succeeds and logs, or halts
//! with diagnostics. There is no "continue on error" path during boot.

#![no_std]
#![no_main]

extern crate alloc;

use core::arch::global_asm;
use core::fmt::Write;
use core::panic::PanicInfo;

mod arch;
mod boot;
mod drivers;
mod exceptions;
mod fs;
mod logging;
mod mm;
mod sched;
mod storage;
mod sync;
mod syscall;
mod time;
mod user;

pub const VERSION: &str = "0.2.0 (core milestone)";

global_asm!(
    r#"
    .section .text.entry, "ax"
    .global _start
_start:
    cli
    cld
    lea rsp, [rip + __stack_top]
    xor rbp, rbp
    call kernel_main
1:
    cli
    hlt
    jmp 1b

    .section .bss
    .balign 16
__stack_bottom:
    .skip 65536
__stack_top:
"#
);

#[no_mangle]
pub extern "C" fn kernel_main(boot_info_address: u64) -> ! {
    logging::init();
    logging::write_raw(b"\n");
    logging::write_raw(b"================================================\n");
    logging::write_raw(b" Aster Kernel ");
    logging::write_raw(VERSION.as_bytes());
    logging::write_raw(b"\n Independent x86_64 kernel\n");
    logging::write_raw(b"================================================\n");
    logging::write_raw(b"KERNEL-ENTRY-OK\n");

    let boot = boot::init(boot_info_address);
    time::init(boot.boot_tsc);

    let (vendor, max_leaf) = arch::cpu::vendor();
    let vendor = core::str::from_utf8(&vendor).unwrap_or("unknown");
    crate::info!("CPU: {} (cpuid max leaf {})", vendor.trim(), max_leaf);
    crate::info!(
        "bootinfo {:#x}: {} MiB RAM region, framebuffer {}x{}",
        boot_info_address,
        boot::usable_memory_end() / (1024 * 1024),
        boot.framebuffer.width,
        boot.framebuffer.height
    );

    unsafe {
        arch::gdt::init();
        crate::info!("gdt: kernel/user segments installed, TSS loaded");
        arch::idt::init();
        crate::info!("idt: 256 vectors, syscall gate DPL=3");
        arch::pic::init();
        crate::info!("pic: remapped to 32..47 (timer, keyboard, cascade, mouse)");
        time::start_pit();
        arch::cpu::init_fpu();
        crate::info!("pit: {} Hz, SSE enabled for user space", time::TIMER_HZ);
    }

    mm::pmm::init();
    mm::vmm::init();
    mm::heap::init();

    if !mm::heap::self_test() {
        crate::error!("heap self-test failed");
        halt_with_diagnostics();
    }
    crate::info!("heap: self-test passed");

    if !mm::selftest::run() {
        crate::error!("memory self-test failed");
        halt_with_diagnostics();
    }
    crate::info!("mm: mapping and user address-space tests passed");

    storage::init();
    fs::init();
    if !fs::self_test() {
        crate::error!("ramfs self-test failed");
        halt_with_diagnostics();
    }
    crate::info!("ramfs: self-test passed");

    sched::init();
    drivers::ps2::init();
    match user::spawn_module("desk") {
        Ok(pid) => crate::info!("init: desktop started as pid {}", pid),
        Err(error) => crate::error!("init: cannot start the desktop: {}", error),
    }

    let (total_kib, free_kib) = mm::pmm::stats();
    let (heap_mapped, heap_used) = mm::heap::stats_kib();
    crate::info!(
        "memory: {} MiB RAM, {} MiB free, heap {} KiB ({} KiB used)",
        total_kib / 1024,
        free_kib / 1024,
        heap_mapped,
        heap_used
    );
    crate::info!(
        "kernel address space: image {:#x}, direct map {:#x}",
        mm::vmm::KERNEL_BASE,
        mm::vmm::HHDM_OFFSET
    );
    crate::info!("KERNEL-READY");

    // The idle thread owns this CPU from here on; this call never returns.
    sched::start();
}

fn halt_with_diagnostics() -> ! {
    crate::error!("boot failed; system halted");
    loop {
        arch::cpu::disable_interrupts();
        arch::cpu::halt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    arch::cpu::disable_interrupts();
    logging::write_raw(b"\n*** KERNEL PANIC ***\n");
    let mut writer = PanicWriter;
    let _ = write!(writer, "{}\n", info);
    logging::write_raw(b"system halted\n");
    loop {
        arch::cpu::halt();
    }
}

struct PanicWriter;
impl Write for PanicWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        logging::write_raw(s.as_bytes());
        Ok(())
    }
}
