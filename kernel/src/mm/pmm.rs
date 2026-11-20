//! Physical memory manager: 4 KiB frame bitmap allocator.
//!
//! The bitmap is a static array supporting up to 32 GiB of physical memory.
//! Frame 0 is never returned; the first megabyte is reserved for firmware
//! structures and real-mode compatibility.

use aster_abi::MemoryType;
use crate::boot;
use crate::sync::{IrqLock, SyncUnsafeCell};

pub const PAGE_SIZE: usize = 4096;
const MAX_FRAMES: usize = 8 * 1024 * 1024; // 32 GiB
const WORDS: usize = MAX_FRAMES / 64;

static BITMAP: SyncUnsafeCell<[u64; WORDS]> = SyncUnsafeCell::new([u64::MAX; WORDS]);

struct State {
    total: usize,
    free: usize,
    hint: usize,
}

static STATE: IrqLock<State> = IrqLock::new(State {
    total: 0,
    free: 0,
    hint: 0,
});

#[inline]
unsafe fn is_used(frame: usize) -> bool {
    let bitmap = BITMAP.get();
    ((*bitmap)[frame / 64] >> (frame % 64)) & 1 != 0
}

#[inline]
unsafe fn set_used(frame: usize, used: bool) {
    let bitmap = BITMAP.get();
    let mask = 1u64 << (frame % 64);
    if used {
        (*bitmap)[frame / 64] |= mask;
    } else {
        (*bitmap)[frame / 64] &= !mask;
    }
}

unsafe fn reserve(start: u64, end: u64) {
    let first = (start as usize) / PAGE_SIZE;
    let last = ((end as usize) + PAGE_SIZE - 1) / PAGE_SIZE;
    let mut state = STATE.lock();
    for frame in first..last.min(MAX_FRAMES) {
        if !is_used(frame) {
            set_used(frame, true);
            state.free = state.free.saturating_sub(1);
        }
    }
}

pub fn init() {
    let boot = boot::info();
    // Only usable RAM establishes the frame range; MMIO apertures can live at
    // very high physical addresses and must not inflate the bitmap.
    let mut highest = boot::usable_memory_end() as usize;
    highest = highest.max((boot.kernel_phys_start + boot.kernel_phys_size) as usize);
    highest = (highest + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    highest = highest.min(MAX_FRAMES * PAGE_SIZE);
    let total_frames = highest / PAGE_SIZE;

    unsafe {
        for frame in 0..total_frames {
            set_used(frame, true);
        }
        {
            let mut state = STATE.lock();
            state.total = total_frames;
            state.free = 0;
            state.hint = 0;
        }

        for descriptor in boot::descriptors() {
            if boot::memory_type(descriptor) != MemoryType::Conventional {
                continue;
            }
            let start = descriptor.physical_start as usize;
            let end = start + descriptor.number_of_pages as usize * PAGE_SIZE;
            let first = start / PAGE_SIZE;
            let last = (end / PAGE_SIZE).min(total_frames);
            if first >= last {
                continue;
            }
            let mut state = STATE.lock();
            for frame in first..last {
                if is_used(frame) {
                    set_used(frame, false);
                    state.free += 1;
                }
            }
        }

        // Reserve firmware/low memory and every image the loader handed us.
        reserve(0, 0x10_0000);
        reserve(
            boot.kernel_phys_start,
            boot.kernel_phys_start + boot.kernel_phys_size,
        );
        reserve(
            boot.framebuffer.address,
            boot.framebuffer.address + boot.framebuffer.size,
        );
        reserve(boot.self_physical, boot.self_physical + 4096);
        for index in 0..boot.module_count as usize {
            let module = &boot.modules[index];
            reserve(module.address, module.address + module.size);
        }
    }

    let state = STATE.lock();
    crate::info!(
        "pmm: {} MiB usable, {} MiB free, {} frames",
        state.total * PAGE_SIZE / (1024 * 1024),
        state.free * PAGE_SIZE / (1024 * 1024),
        state.total
    );
}

pub fn alloc_frame() -> Option<u64> {
    let mut state = STATE.lock();
    let total = state.total;
    if state.free == 0 {
        return None;
    }
    let start = state.hint.min(total.saturating_sub(1));
    unsafe {
        for offset in 0..total {
            let frame = (start + offset) % total;
            if frame == 0 {
                continue;
            }
            if !is_used(frame) {
                set_used(frame, true);
                state.free -= 1;
                state.hint = (frame + 1) % total;
                return Some((frame * PAGE_SIZE) as u64);
            }
        }
    }
    None
}

pub fn free_frame(address: u64) {
    let frame = (address as usize) / PAGE_SIZE;
    if frame == 0 || frame >= MAX_FRAMES {
        return;
    }
    let mut state = STATE.lock();
    unsafe {
        if is_used(frame) {
            set_used(frame, false);
            state.free += 1;
            state.hint = state.hint.min(frame);
        }
    }
}

pub fn alloc_zeroed_frame() -> Option<u64> {
    let frame = alloc_frame()?;
    unsafe {
        core::ptr::write_bytes(
            boot::physical_to_virtual(frame) as *mut u8,
            0,
            PAGE_SIZE,
        );
    }
    Some(frame)
}

pub fn stats() -> (u64, u64) {
    let state = STATE.lock();
    (
        (state.total * PAGE_SIZE / 1024) as u64,
        (state.free * PAGE_SIZE / 1024) as u64,
    )
}

pub fn free_kib() -> u64 {
    STATE.lock().free as u64 * (PAGE_SIZE / 1024) as u64
}

/// Consume a frame count for accounting tests without leaking correctness:
/// used by the boot self-test.
pub fn audit_used_frames() -> usize {
    let state = STATE.lock();
    state.total - state.free
}

pub fn memory_end() -> u64 {
    STATE.lock().total as u64 * PAGE_SIZE as u64
}

/// Ensure the bitmap is not optimised away by the linker.
pub fn bitmap_address() -> usize {
    BITMAP.get() as usize
}

pub fn _reserve(start: u64, end: u64) {
    unsafe { reserve(start, end) }
}
