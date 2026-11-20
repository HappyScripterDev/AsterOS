//! Kernel heap: first-fit free list over a contiguous virtual region.
//!
//! The heap grows by mapping physical frames into `HEAP_BASE`, so virtual
//! addresses are stable while frames stay scattered. A block header precedes
//! every allocation; the allocator is protected by an interrupt-safe lock.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use crate::mm::pmm;
use crate::mm::vmm;
use crate::sync::IrqLock;

const HEADER_SIZE: usize = 32;
const MIN_SPLIT: usize = 64;
const GROW_SIZE: usize = 1024 * 1024;
const HEAP_LIMIT: usize = 256 * 1024 * 1024;

#[repr(C)]
struct Block {
    size: usize,
    free: bool,
    prev: *mut Block,
    next: *mut Block,
}

struct Heap {
    mapped: usize,
    free_bytes: usize,
    allocated_bytes: usize,
    head: *mut Block,
}

unsafe impl Send for Heap {}

static HEAP: IrqLock<Heap> = IrqLock::new(Heap {
    mapped: 0,
    free_bytes: 0,
    allocated_bytes: 0,
    head: ptr::null_mut(),
});

#[inline]
fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

unsafe fn add_block(heap: &mut Heap, block: *mut Block) {
    if heap.head.is_null() {
        heap.head = block;
        return;
    }
    let mut current = heap.head;
    while !(*current).next.is_null() && (*current).next < block {
        current = (*current).next;
    }
    (*block).prev = current;
    (*block).next = (*current).next;
    if !(*current).next.is_null() {
        (*(*current).next).prev = block;
    }
    (*current).next = block;
}

unsafe fn grow(heap: &mut Heap) -> bool {
    if heap.mapped + GROW_SIZE > HEAP_LIMIT {
        return false;
    }
    let virtual_base = vmm::HEAP_BASE + heap.mapped as u64;
    let pages = GROW_SIZE / pmm::PAGE_SIZE;
    for index in 0..pages {
        let frame = match pmm::alloc_frame() {
            Some(frame) => frame,
            None => return false,
        };
        if let Err(error) = vmm::map_page(
            vmm::kernel_pml4(),
            virtual_base + index as u64 * pmm::PAGE_SIZE as u64,
            frame,
            vmm::PTE_WRITE | vmm::PTE_NX | vmm::PTE_GLOBAL,
        ) {
            pmm::free_frame(frame);
            crate::error!("heap growth failed: {}", error);
            return false;
        }
    }
    let block = virtual_base as *mut Block;
    (*block).size = GROW_SIZE;
    (*block).free = true;
    (*block).prev = ptr::null_mut();
    (*block).next = ptr::null_mut();
    add_block(heap, block);
    heap.mapped += GROW_SIZE;
    heap.free_bytes += GROW_SIZE;
    true
}

pub fn init() {
    let mut heap = HEAP.lock();
    if !unsafe { grow(&mut heap) } {
        panic!("cannot initialise the kernel heap");
    }
    crate::info!(
        "heap: {} KiB mapped at {:#x}",
        heap.mapped / 1024,
        vmm::HEAP_BASE
    );
}

unsafe fn allocate(heap: &mut Heap, size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return ptr::null_mut();
    }
    let align = align.max(16);
    let needed = align_up(size + align + HEADER_SIZE + 8, 16);
    let mut block = heap.head;
    while !block.is_null() {
        if (*block).free && (*block).size >= needed {
            break;
        }
        block = (*block).next;
    }
    if block.is_null() {
        if !grow(heap) {
            return ptr::null_mut();
        }
        block = heap.head;
        while !block.is_null() {
            if (*block).free && (*block).size >= needed {
                break;
            }
            block = (*block).next;
        }
        if block.is_null() {
            return ptr::null_mut();
        }
    }

    if (*block).size - needed >= MIN_SPLIT {
        let remainder = (block as usize + needed) as *mut Block;
        (*remainder).size = (*block).size - needed;
        (*remainder).free = true;
        (*remainder).next = (*block).next;
        (*remainder).prev = block;
        if !(*block).next.is_null() {
            (*(*block).next).prev = remainder;
        }
        (*block).next = remainder;
        (*block).size = needed;
    }

    (*block).free = false;
    heap.free_bytes -= (*block).size;
    heap.allocated_bytes += (*block).size;

    let payload = block as usize + HEADER_SIZE;
    let aligned = align_up(payload + 8, align);
    // Store the owning block immediately before the returned pointer.
    *((aligned - 8) as *mut usize) = block as usize;
    aligned as *mut u8
}

unsafe fn release(heap: &mut Heap, pointer: *mut u8) {
    if pointer.is_null() {
        return;
    }
    let block = *((pointer as usize - 8) as *const usize) as *mut Block;
    if block.is_null() || (*block).free {
        return;
    }
    (*block).free = true;
    heap.free_bytes += (*block).size;
    heap.allocated_bytes = heap.allocated_bytes.saturating_sub((*block).size);

    if !(*block).next.is_null() && (*(*block).next).free {
        let next = (*block).next;
        (*block).size += (*next).size;
        (*block).next = (*next).next;
        if !(*next).next.is_null() {
            (*(*next).next).prev = block;
        }
    }
    if !(*block).prev.is_null() && (*(*block).prev).free {
        let prev = (*block).prev;
        (*prev).size += (*block).size;
        (*prev).next = (*block).next;
        if !(*block).next.is_null() {
            (*(*block).next).prev = prev;
        }
    }
}

pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = HEAP.lock();
        allocate(&mut heap, layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        let mut heap = HEAP.lock();
        release(&mut heap, pointer);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = self.alloc(layout);
        if !pointer.is_null() {
            ptr::write_bytes(pointer, 0, layout.size());
        }
        pointer
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;

pub fn stats_kib() -> (u64, u64) {
    let heap = HEAP.lock();
    (
        (heap.mapped / 1024) as u64,
        (heap.allocated_bytes / 1024) as u64,
    )
}

/// Self-test: allocate patterns, verify isolation, free and reallocate.
pub fn self_test() -> bool {
    let mut values = alloc::vec::Vec::new();
    for i in 0..256usize {
        values.push(alloc::vec![i as u8; 64 + (i % 97)]);
    }
    for (i, value) in values.iter().enumerate() {
        if value.len() != 64 + (i % 97) || value[0] != i as u8 || value[value.len() - 1] != i as u8 {
            crate::error!("heap self-test: pattern mismatch at {}", i);
            return false;
        }
    }
    drop(values);
    let before = HEAP.lock().allocated_bytes;
    let large = alloc::vec![0xA5u8; 512 * 1024];
    if large[12345] != 0xA5 {
        return false;
    }
    drop(large);
    let after = HEAP.lock().allocated_bytes;
    if after > before {
        crate::error!("heap self-test: leak detected ({} -> {})", before, after);
        return false;
    }
    true
}
