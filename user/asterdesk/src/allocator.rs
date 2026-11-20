//! Desktop heap: bump allocator over a single anonymous mapping.
//!
//! The desktop is single-threaded, so a lock-free bump allocator is
//! sufficient and fast. `dealloc` is a no-op; long-running applications should
//! eventually get a real allocator (tracked in docs/PROJECT_STATUS.md).

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::sys;

static START: AtomicUsize = AtomicUsize::new(0);
static OFFSET: AtomicUsize = AtomicUsize::new(0);
static SIZE: AtomicUsize = AtomicUsize::new(0);

pub fn init(bytes: usize) -> bool {
    match sys::vm_alloc(bytes) {
        Some(address) => {
            START.store(address as usize, Ordering::Relaxed);
            SIZE.store(bytes, Ordering::Relaxed);
            OFFSET.store(0, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

struct Bump;

unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let start = START.load(Ordering::Relaxed);
        let size = SIZE.load(Ordering::Relaxed);
        if start == 0 || size == 0 {
            return core::ptr::null_mut();
        }
        let mut offset = OFFSET.load(Ordering::Relaxed);
        loop {
            let aligned = (offset + layout.align() - 1) & !(layout.align() - 1);
            let end = aligned + layout.size();
            if end > size {
                return core::ptr::null_mut();
            }
            match OFFSET.compare_exchange_weak(offset, end, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return (start + aligned) as *mut u8,
                Err(current) => offset = current,
            }
        }
    }

    unsafe fn dealloc(&self, _pointer: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOCATOR: Bump = Bump;
