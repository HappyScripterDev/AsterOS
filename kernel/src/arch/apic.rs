//! Local APIC and IOAPIC bring-up for the single-CPU platform.
//!
//! The 8259 path remains available as a fallback: if the MADT has no LAPIC or
//! IOAPIC, `init()` leaves the PIC active. When APIC mode is active, external
//! interrupts are acknowledged through the Local APIC and the PIC is masked.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};

use crate::arch::acpi;
use crate::sync::IrqLock;

const APIC_BASE_MSR: u32 = 0x1B;
const REG_ID: u64 = 0x20;
const REG_TPR: u64 = 0x80;
const REG_EOI: u64 = 0xB0;
const REG_SVR: u64 = 0xF0;
const SPURIOUS_VECTOR: u32 = 47;

const IOAPIC_REGSEL: u64 = 0x00;
const IOAPIC_WIN: u64 = 0x10;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LAPIC: AtomicU64 = AtomicU64::new(0);
static IOAPIC: AtomicU64 = AtomicU64::new(0);
static IOAPIC_ENTRIES: AtomicU8 = AtomicU8::new(0);
static EOI_COUNT: AtomicU32 = AtomicU32::new(0);
static TIMER_SEEN: AtomicBool = AtomicBool::new(false);
static KEYBOARD_SEEN: AtomicBool = AtomicBool::new(false);
static MOUSE_SEEN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct Route {
    irq: u8,
    vector: u8,
    seen: &'static AtomicBool,
}

const ROUTES: [Route; 3] = [
    Route {
        irq: 0,
        vector: 32,
        seen: &TIMER_SEEN,
    },
    Route {
        irq: 1,
        vector: 33,
        seen: &KEYBOARD_SEEN,
    },
    Route {
        irq: 12,
        vector: 44,
        seen: &MOUSE_SEEN,
    },
];

fn lapic_read(offset: u64) -> u32 {
    let base = LAPIC.load(Ordering::Relaxed);
    if base == 0 {
        return 0;
    }
    unsafe { ((base + offset) as *const u32).read_volatile() }
}

fn lapic_write(offset: u64, value: u32) {
    let base = LAPIC.load(Ordering::Relaxed);
    if base != 0 {
        unsafe { ((base + offset) as *mut u32).write_volatile(value) };
    }
}

fn ioapic_read(register: u32) -> u32 {
    let base = IOAPIC.load(Ordering::Relaxed);
    if base == 0 {
        return 0;
    }
    unsafe {
        ((base + IOAPIC_REGSEL) as *mut u32).write_volatile(register);
        ((base + IOAPIC_WIN) as *const u32).read_volatile()
    }
}

fn ioapic_write(register: u32, value: u32) {
    let base = IOAPIC.load(Ordering::Relaxed);
    if base == 0 {
        return;
    }
    unsafe {
        ((base + IOAPIC_REGSEL) as *mut u32).write_volatile(register);
        ((base + IOAPIC_WIN) as *mut u32).write_volatile(value);
    }
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub fn eoi() {
    lapic_write(REG_EOI, 0);
    EOI_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn eoi_count() -> u32 {
    EOI_COUNT.load(Ordering::Relaxed)
}

pub fn backend() -> &'static str {
    if active() {
        "apic"
    } else {
        "pic"
    }
}

/// Note a routed interrupt for the one-time proof logs and Monitor counters.
pub fn note_vector(vector: u64) {
    for route in ROUTES {
        if route.vector as u64 == vector && !route.seen.swap(true, Ordering::Relaxed) {
            crate::info!(
                "apic: first IRQ irq {} gsi {} vector {}",
                route.irq,
                acpi::gsi_for_irq(route.irq).0,
                route.vector
            );
        }
    }
}

fn ioapic_set(entry: u32, low: u32, high: u32) {
    ioapic_write(0x10 + entry * 2, low);
    ioapic_write(0x11 + entry * 2, high);
}

fn route_irq(irq: u8, vector: u8) -> bool {
    let (gsi, flags) = acpi::gsi_for_irq(irq);
    if gsi >= IOAPIC_ENTRIES.load(Ordering::Relaxed) as u32 {
        crate::warn!("ioapic: gsi {} for irq {} is out of range", gsi, irq);
        return false;
    }
    let polarity = if flags & 0x2 != 0 { 1 } else { 0 };
    let trigger = if flags & 0x8 != 0 { 1 } else { 0 };
    let mut low = vector as u32 | (polarity << 13) | (trigger << 15);
    // Destination physical, fixed delivery, unmasked.
    let destination = (lapic_read(REG_ID) >> 24) & 0xFF;
    let high = destination << 24;
    ioapic_set(gsi, low, high);
    crate::info!(
        "ioapic: irq {} -> gsi {} vector {} {}{}",
        irq,
        gsi,
        vector,
        if polarity == 1 {
            "active-low "
        } else {
            "active-high "
        },
        if trigger == 1 { "level" } else { "edge" }
    );
    true
}

pub fn init() -> bool {
    let state = acpi::state();
    if state.lapic_base == 0 || state.ioapic_count == 0 {
        crate::warn!("apic: MADT lacks a LAPIC or IOAPIC; keeping the 8259 path");
        return false;
    }
    let lapic_virtual = match crate::mm::vmm::map_kernel_mmio(state.lapic_base, 0x1000) {
        Ok(address) => address,
        Err(error) => {
            crate::error!("apic: cannot map LAPIC: {}", error);
            return false;
        }
    };
    let ioapic_address = state.ioapics[0].address as u64;
    let ioapic_virtual = match crate::mm::vmm::map_kernel_mmio(ioapic_address, 0x1000) {
        Ok(address) => address,
        Err(error) => {
            crate::error!("ioapic: cannot map IOAPIC: {}", error);
            return false;
        }
    };
    LAPIC.store(lapic_virtual, Ordering::Relaxed);
    IOAPIC.store(ioapic_virtual, Ordering::Relaxed);

    unsafe {
        let base = crate::arch::cpu::rdmsr(APIC_BASE_MSR);
        crate::arch::cpu::wrmsr(APIC_BASE_MSR, (base & !0xFFF) | (1 << 11));
    }
    lapic_write(REG_TPR, 0);
    // Spurious vector 47 + APIC software-enable bit 8.
    lapic_write(REG_SVR, SPURIOUS_VECTOR | (1 << 8));

    let version = ioapic_read(0x01);
    let entries = ((version >> 16) & 0xFF) as u8;
    IOAPIC_ENTRIES.store(entries, Ordering::Relaxed);
    crate::info!(
        "apic: LAPIC {:#x} id {}, IOAPIC {:#x} version {:#x}, {} redirections",
        state.lapic_base,
        (lapic_read(REG_ID) >> 24) & 0xFF,
        ioapic_address,
        version & 0xFF,
        entries
    );
    if entries == 0 {
        crate::error!("ioapic: reported zero redirection entries");
        return false;
    }

    // Mask every redirection first, then unmask only the three legacy ISA
    // sources the kernel actually handles.
    for entry in 0..entries as u32 {
        ioapic_set(entry, 1 << 16, 0);
    }
    let mut routed = 0;
    for route in ROUTES {
        if route_irq(route.irq, route.vector) {
            routed += 1;
        }
    }
    if routed != ROUTES.len() {
        crate::warn!(
            "ioapic: only {}/{} ISA routes programmed",
            routed,
            ROUTES.len()
        );
        return false;
    }

    unsafe {
        crate::arch::pic::mask_all();
    }
    ACTIVE.store(true, Ordering::Relaxed);
    crate::info!(
        "apic: active, PIC masked, EOI -> LAPIC, spurious vector {}",
        SPURIOUS_VECTOR
    );
    true
}
