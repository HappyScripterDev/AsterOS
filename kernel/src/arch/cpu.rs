//! CPU control-register and low-level helpers.

use core::arch::asm;

pub fn interrupts_enabled() -> bool {
    let flags: u64;
    unsafe {
        asm!("pushfq", "pop {}", out(reg) flags, options(nomem, preserves_flags));
    }
    flags & (1 << 9) != 0
}

pub fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

pub fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

pub fn halt() {
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

pub fn idle_loop() -> ! {
    loop {
        unsafe {
            asm!("sti", "hlt", options(nomem, nostack));
        }
    }
}

pub fn read_cr2() -> u64 {
    let value;
    unsafe {
        asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

pub fn read_cr3() -> u64 {
    let value;
    unsafe {
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

pub unsafe fn write_cr3(value: u64) {
    asm!("mov cr3, {}", in(reg) value, options(nomem, nostack, preserves_flags));
}

pub fn read_cr0() -> u64 {
    let value;
    unsafe {
        asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

pub fn read_cr4() -> u64 {
    let value;
    unsafe {
        asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Enable SSE so user space can use floating point. The kernel itself is
/// compiled without FP; per-thread FXSAVE state is handled by the scheduler.
/// Also enables NX and write-protect so the W^X mappings mean something.
pub unsafe fn init_fpu() {
    let cr0 = read_cr0();
    // Clear EM (bit 2) and TS (bit 3); set MP (bit 1), NE (bit 5) and WP
    // (bit 16) so ring 0 cannot silently write read-only pages.
    let cr0 = (cr0 & !((1 << 2) | (1 << 3))) | (1 << 1) | (1 << 5) | (1 << 16);
    asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
    let cr4 = read_cr4();
    // OSFXSR | OSXMMEXCPT
    let cr4 = cr4 | (1 << 9) | (1 << 10);
    asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));

    // IA32_EFER.NXE is required for the NX bits installed by the loader.
    const IA32_EFER: u32 = 0xC000_0080;
    let mut low: u32;
    let mut high: u32;
    asm!("rdmsr", in("ecx") IA32_EFER, out("eax") low, out("edx") high, options(nomem, nostack));
    let mut efer = ((high as u64) << 32) | low as u64;
    if efer & (1 << 11) == 0 {
        efer |= 1 << 11;
        low = efer as u32;
        high = (efer >> 32) as u32;
        asm!("wrmsr", in("ecx") IA32_EFER, in("eax") low, in("edx") high, options(nomem, nostack));
    }
}

/// Read the CPU vendor string and maximum basic leaf. Used in boot
/// diagnostics and to gate future features.
pub fn vendor() -> ([u8; 12], u32) {
    let mut ebx_value: u32 = 0;
    let mut ecx: u32 = 0;
    let mut edx: u32 = 0;
    let mut max_leaf: u32 = 0;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov [{ebx_ptr}], ebx",
            "pop rbx",
            ebx_ptr = in(reg) &mut ebx_value,
            inout("eax") 0u32 => max_leaf,
            out("ecx") ecx,
            out("edx") edx,
            options(nostack, preserves_flags)
        );
    }
    let mut bytes = [0u8; 12];
    bytes[0..4].copy_from_slice(&ebx_value.to_le_bytes());
    bytes[4..8].copy_from_slice(&edx.to_le_bytes());
    bytes[8..12].copy_from_slice(&ecx.to_le_bytes());
    (bytes, max_leaf)
}

/// Read a model-specific register.
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    core::arch::asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack));
    ((high as u64) << 32) | low as u64
}

/// Write a model-specific register.
pub unsafe fn wrmsr(msr: u32, value: u64) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u32,
        options(nomem, nostack)
    );
}
