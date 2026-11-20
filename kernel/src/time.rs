//! Timekeeping: PIT-driven millisecond tick counter.

use core::sync::atomic::{AtomicU64, Ordering};

pub const TIMER_HZ: u32 = 1000;

static TICKS: AtomicU64 = AtomicU64::new(0);
static BOOT_TSC: AtomicU64 = AtomicU64::new(0);

pub fn init(boot_tsc: u64) {
    BOOT_TSC.store(boot_tsc, Ordering::Relaxed);
}

pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn uptime_ms() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn uptime_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn boot_tsc() -> u64 {
    BOOT_TSC.load(Ordering::Relaxed)
}

pub fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Program the 8254 PIT channel 0 to `TIMER_HZ` (approximately 1000 Hz).
pub unsafe fn start_pit() {
    const PIT_FREQUENCY: u32 = 1_193_182;
    let divisor = (PIT_FREQUENCY / TIMER_HZ) as u16;

    core::arch::asm!("out 0x43, al", in("al") 0x36u8, options(nomem, nostack, preserves_flags));
    core::arch::asm!("out 0x40, al", in("al") (divisor & 0xFF) as u8, options(nomem, nostack, preserves_flags));
    core::arch::asm!("out 0x40, al", in("al") (divisor >> 8) as u8, options(nomem, nostack, preserves_flags));
}
