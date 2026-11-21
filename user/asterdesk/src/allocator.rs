//! Desktop heap: first-fit allocator over one anonymous mapping.
//!
//! The desktop is single-threaded, so the free list needs no locking; the
//! atomics only satisfy `GlobalAlloc`'s `Sync` bound. Blocks are 16-byte
//! aligned, carry a 16-byte header, split on allocation and coalesce with
//! neighbours when freed. This replaces the original bump allocator whose
//! `dealloc` was a no-op and leaked every transient buffer.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::sys;

const HEADER: usize = 16;
const MIN_SPLIT: usize = HEADER + 16;
const ALIGN: usize = 16;

static START: AtomicUsize = AtomicUsize::new(0);
static SIZE: AtomicUsize = AtomicUsize::new(0);
static BUMP: AtomicUsize = AtomicUsize::new(0);
static FREE: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct Block {
    /// Total block size including this header, rounded to `ALIGN`.
    size: usize,
    /// Next free block address, or 0.
    next: usize,
}

impl Block {
    unsafe fn at(address: usize) -> *mut Block {
        address as *mut Block
    }
}

pub fn init(bytes: usize) -> bool {
    match sys::vm_alloc(bytes & !(ALIGN - 1)) {
        Some(address) => {
            START.store(address as usize, Ordering::Relaxed);
            SIZE.store(bytes & !(ALIGN - 1), Ordering::Relaxed);
            BUMP.store(0, Ordering::Relaxed);
            FREE.store(0, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Insert `address` into the address-sorted free list, merging with the
/// previous and next block when they are physically adjacent.
unsafe fn insert_free(address: usize) {
    let mut previous = 0usize;
    let mut current = FREE.load(Ordering::Relaxed);
    while current != 0 && current < address {
        previous = current;
        current = (*Block::at(current)).next;
    }

    let mut size = (*Block::at(address)).size;
    let mut next = current;
    if current != 0 && address + size == current {
        size += (*Block::at(current)).size;
        next = (*Block::at(current)).next;
    }
    (*Block::at(address)).size = size;

    if previous != 0 && previous + (*Block::at(previous)).size == address {
        (*Block::at(previous)).size += size;
        (*Block::at(previous)).next = next;
    } else {
        (*Block::at(address)).next = next;
        if previous == 0 {
            FREE.store(address, Ordering::Relaxed);
        } else {
            (*Block::at(previous)).next = address;
        }
    }
}

struct Heap;

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let start = START.load(Ordering::Relaxed);
        let size = SIZE.load(Ordering::Relaxed);
        if start == 0 || size == 0 || layout.align() > ALIGN {
            return core::ptr::null_mut();
        }
        let needed = (layout.size() + HEADER + ALIGN - 1) & !(ALIGN - 1);
        let needed = needed.max(MIN_SPLIT);

        // First fit in the free list.
        let mut previous = 0usize;
        let mut current = FREE.load(Ordering::Relaxed);
        while current != 0 {
            let block = Block::at(current);
            let block_size = (*block).size;
            if block_size >= needed {
                let next = (*block).next;
                if previous == 0 {
                    FREE.store(next, Ordering::Relaxed);
                } else {
                    (*Block::at(previous)).next = next;
                }
                let remainder = block_size - needed;
                if remainder >= MIN_SPLIT {
                    (*block).size = needed;
                    let rest = Block::at(current + needed);
                    (*rest).size = remainder;
                    (*rest).next = 0;
                    insert_free(current + needed);
                }
                return (current + HEADER) as *mut u8;
            }
            previous = current;
            current = (*block).next;
        }

        // Bump the tail.
        let mut offset = BUMP.load(Ordering::Relaxed);
        loop {
            let aligned = (offset + ALIGN - 1) & !(ALIGN - 1);
            let end = match aligned.checked_add(needed) {
                Some(end) if end <= size => end,
                _ => return core::ptr::null_mut(),
            };
            match BUMP.compare_exchange_weak(offset, end, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => {
                    let block = Block::at(start + aligned);
                    (*block).size = needed;
                    (*block).next = 0;
                    return (start + aligned + HEADER) as *mut u8;
                }
                Err(current) => offset = current,
            }
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        if pointer.is_null() {
            return;
        }
        let address = pointer as usize - HEADER;
        let start = START.load(Ordering::Relaxed);
        let size = SIZE.load(Ordering::Relaxed);
        if address < start || address + HEADER > start + size {
            return;
        }
        insert_free(address);
    }
}

#[global_allocator]
static ALLOCATOR: Heap = Heap;
