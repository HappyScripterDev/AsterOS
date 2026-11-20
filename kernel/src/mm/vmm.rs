//! Virtual memory manager: 4-level paging helpers.
//!
//! The kernel is higher-half: PML4[256] is the direct physical map, PML4[511]
//! is the kernel image. User processes receive a fresh PML4 whose kernel half
//! is shared, with private mappings in PML4[0..256].

use core::sync::atomic::{AtomicU64, Ordering};

use crate::boot;
use crate::mm::pmm;

pub const PAGE_SIZE: u64 = 4096;
pub const HHDM_OFFSET: u64 = 0xFFFF_8000_0000_0000;
pub const KERNEL_BASE: u64 = 0xFFFF_FFFF_8000_0000;
pub const HEAP_BASE: u64 = 0xFFFF_9000_0000_0000;
pub const USER_BASE: u64 = 0x0000_0000_0040_0000;
pub const USER_STACK_TOP: u64 = USER_BASE + 0x0000_0000_4000_0000;
pub const USER_TOP: u64 = USER_BASE + 0x0000_0000_8000_0000;

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_WRITETHROUGH: u64 = 1 << 3;
pub const PTE_CACHE_DISABLE: u64 = 1 << 4;
pub const PTE_ACCESSED: u64 = 1 << 5;
pub const PTE_DIRTY: u64 = 1 << 6;
pub const PTE_HUGE: u64 = 1 << 7;
pub const PTE_GLOBAL: u64 = 1 << 8;
pub const PTE_NX: u64 = 1 << 63;

pub const ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;

static KERNEL_PML4: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    let cr3 = crate::arch::cpu::read_cr3() & ADDRESS_MASK;
    KERNEL_PML4.store(cr3, Ordering::Relaxed);
    crate::info!(
        "vmm: kernel PML4 at {:#x}, user space starts at {:#x}",
        cr3,
        USER_BASE
    );
}

pub fn kernel_pml4() -> u64 {
    KERNEL_PML4.load(Ordering::Relaxed)
}

#[inline]
pub fn hhdm(physical: u64) -> u64 {
    HHDM_OFFSET + physical
}

#[inline]
unsafe fn entry(physical_table: u64, index: usize) -> *mut u64 {
    (hhdm(physical_table) as *mut u64).add(index)
}

unsafe fn table_from_entry(value: u64) -> u64 {
    value & ADDRESS_MASK
}

unsafe fn ensure_table(entry_ptr: *mut u64, label: &'static str) -> Result<u64, &'static str> {
    let value = *entry_ptr;
    if value & PTE_PRESENT != 0 {
        if value & PTE_HUGE != 0 {
            return Err("page walk encountered a huge page");
        }
        return Ok(table_from_entry(value));
    }
    let frame = pmm::alloc_zeroed_frame().ok_or(label)?;
    *entry_ptr = frame | PTE_PRESENT | PTE_WRITE;
    Ok(frame)
}

pub fn map_page(pml4: u64, virtual_address: u64, physical_address: u64, flags: u64) -> Result<(), &'static str> {
    if virtual_address & 0xFFF != 0 || physical_address & 0xFFF != 0 {
        return Err("unaligned mapping request");
    }
    unsafe {
        let user = flags & PTE_USER != 0;
        let pml4e = entry(pml4, ((virtual_address >> 39) & 0x1FF) as usize);
        let pdpt = ensure_table(pml4e, "out of memory: PDPT")?;
        let pdpte = entry(pdpt, ((virtual_address >> 30) & 0x1FF) as usize);
        let pd = ensure_table(pdpte, "out of memory: PD")?;
        let pde = entry(pd, ((virtual_address >> 21) & 0x1FF) as usize);
        let pt = ensure_table(pde, "out of memory: PT")?;
        if user {
            *pml4e |= PTE_USER;
            *pdpte |= PTE_USER;
            *pde |= PTE_USER;
        }
        let pte = entry(pt, ((virtual_address >> 12) & 0x1FF) as usize);
        if *pte & PTE_PRESENT != 0 {
            return Err("address already mapped");
        }
        *pte = (physical_address & ADDRESS_MASK) | flags | PTE_PRESENT;
    }
    Ok(())
}

pub fn map_pages(    pml4: u64,
    virtual_address: u64,
    physical_address: u64,
    length: usize,
    flags: u64,
) -> Result<(), &'static str> {
    let pages = (length + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
    for index in 0..pages {
        let offset = index as u64 * PAGE_SIZE;
        map_page(
            pml4,
            virtual_address + offset,
            physical_address + offset,
            flags,
        )?;
    }
    Ok(())
}

pub fn unmap_page(pml4: u64, virtual_address: u64) -> Result<(), &'static str> {
    if virtual_address & 0xFFF != 0 {
        return Err("unaligned unmap request");
    }
    unsafe {
        let pml4e = *entry(pml4, ((virtual_address >> 39) & 0x1FF) as usize);
        if pml4e & PTE_PRESENT == 0 {
            return Err("unmap: not mapped");
        }
        let pdpt = table_from_entry(pml4e);
        let pdpte = *entry(pdpt, ((virtual_address >> 30) & 0x1FF) as usize);
        if pdpte & PTE_PRESENT == 0 || pdpte & PTE_HUGE != 0 {
            return Err("unmap: not mapped");
        }
        let pd = table_from_entry(pdpte);
        let pde = *entry(pd, ((virtual_address >> 21) & 0x1FF) as usize);
        if pde & PTE_PRESENT == 0 || pde & PTE_HUGE != 0 {
            return Err("unmap: not mapped");
        }
        let pt = table_from_entry(pde);
        let pte = entry(pt, ((virtual_address >> 12) & 0x1FF) as usize);
        if *pte & PTE_PRESENT == 0 {
            return Err("unmap: not mapped");
        }
        *pte = 0;
        core::arch::asm!("invlpg [{}]", in(reg) virtual_address, options(nostack, preserves_flags));
    }
    Ok(())
}

pub fn translate(pml4: u64, virtual_address: u64) -> Option<u64> {    unsafe {
        let pml4e = *entry(pml4, ((virtual_address >> 39) & 0x1FF) as usize);
        if pml4e & PTE_PRESENT == 0 {
            return None;
        }
        let pdpt = table_from_entry(pml4e);
        let pdpte = *entry(pdpt, ((virtual_address >> 30) & 0x1FF) as usize);
        if pdpte & PTE_PRESENT == 0 {
            return None;
        }
        if pdpte & PTE_HUGE != 0 {
            return Some((pdpte & ADDRESS_MASK) + (virtual_address & 0x3FFF_FFFF));
        }
        let pd = table_from_entry(pdpte);
        let pde = *entry(pd, ((virtual_address >> 21) & 0x1FF) as usize);
        if pde & PTE_PRESENT == 0 {
            return None;
        }
        if pde & PTE_HUGE != 0 {
            return Some((pde & ADDRESS_MASK) + (virtual_address & 0x1F_FFFF));
        }
        let pt = table_from_entry(pde);
        let pte = *entry(pt, ((virtual_address >> 12) & 0x1FF) as usize);
        if pte & PTE_PRESENT == 0 {
            return None;
        }
        Some((pte & ADDRESS_MASK) + (virtual_address & 0xFFF))
    }
}

/// Create an address space for a user process. Kernel mappings are shared;
/// user space starts empty.
pub fn create_user_pml4() -> Option<u64> {
    let frame = pmm::alloc_zeroed_frame()?;
    let kernel = kernel_pml4();
    unsafe {
        let source = hhdm(kernel) as *const u64;
        let destination = hhdm(frame) as *mut u64;
        for index in 256..512 {
            *destination.add(index) = *source.add(index);
        }
    }
    Some(frame)
}

pub fn destroy_user_pml4(pml4: u64) {
    if pml4 == 0 || pml4 == kernel_pml4() {
        return;
    }
    unsafe {
        for i4 in 0..128 {
            let pml4e = *entry(pml4, i4);
            if pml4e & PTE_PRESENT == 0 {
                continue;
            }
            let pdpt = table_from_entry(pml4e);
            for i3 in 0..512 {
                let pdpte = *entry(pdpt, i3);
                if pdpte & PTE_PRESENT == 0 || pdpte & PTE_HUGE != 0 {
                    continue;
                }
                let pd = table_from_entry(pdpte);
                for i2 in 0..512 {
                    let pde = *entry(pd, i2);
                    if pde & PTE_PRESENT == 0 || pde & PTE_HUGE != 0 {
                        continue;
                    }
                    let pt = table_from_entry(pde);
                    for i1 in 0..512 {
                        let pte = *entry(pt, i1);
                        if pte & PTE_PRESENT != 0 {
                            pmm::free_frame(pte & ADDRESS_MASK);
                        }
                    }
                    pmm::free_frame(pt);
                }
                pmm::free_frame(pd);
            }
            pmm::free_frame(pdpt);
        }
        pmm::free_frame(pml4);
    }
}

/// Map the boot framebuffer into a process address space and return its VA.
pub fn map_framebuffer(pml4: u64, physical: u64, size: u64) -> Result<u64, &'static str> {
    let pages = ((size + PAGE_SIZE - 1) / PAGE_SIZE) as u64;
    let virtual_address = USER_BASE + 0x0000_0000_5000_0000;
    for page in 0..pages {
        let offset = page * PAGE_SIZE;
        map_page(
            pml4,
            virtual_address + offset,
            physical + offset,
            PTE_WRITE | PTE_USER | PTE_WRITETHROUGH | PTE_NX,
        )?;
    }
    Ok(virtual_address)
}

/// Allocate `length` bytes of anonymous, zeroed user memory. Returns the
/// virtual address or an error string.
pub fn user_alloc(
    pml4: u64,
    cursor: &mut u64,
    length: usize,
    flags: u64,
) -> Result<u64, &'static str> {
    if length == 0 {
        return Err("zero-length allocation");
    }
    let pages = (length + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;
    let virtual_address = (*cursor + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let end = virtual_address + pages as u64 * PAGE_SIZE;
    if end > USER_TOP {
        return Err("user address space exhausted");
    }
    for index in 0..pages {
        let frame = pmm::alloc_zeroed_frame().ok_or("out of physical memory")?;
        if let Err(error) = map_page(
            pml4,
            virtual_address + index as u64 * PAGE_SIZE,
            frame,
            flags | PTE_USER,
        ) {
            pmm::free_frame(frame);
            return Err(error);
        }
    }
    *cursor = end;
    Ok(virtual_address)
}

pub fn copy_from_user(pml4: u64, source: u64, destination: &mut [u8]) -> Result<(), &'static str> {
    let mut offset = 0usize;
    while offset < destination.len() {
        let virtual_address = source + offset as u64;
        if virtual_address >= USER_TOP {
            return Err("user pointer outside user space");
        }
        let physical = translate(pml4, virtual_address).ok_or("user pointer not mapped")?;
        let page_offset = (virtual_address & 0xFFF) as usize;
        let chunk = (PAGE_SIZE as usize - page_offset).min(destination.len() - offset);
        unsafe {
            core::ptr::copy_nonoverlapping(
                hhdm(physical) as *const u8,
                destination.as_mut_ptr().add(offset),
                chunk,
            );
        }
        offset += chunk;
    }
    Ok(())
}

pub fn copy_to_user(pml4: u64, destination: u64, source: &[u8]) -> Result<(), &'static str> {
    let mut offset = 0usize;
    while offset < source.len() {
        let virtual_address = destination + offset as u64;
        if virtual_address >= USER_TOP {
            return Err("user pointer outside user space");
        }
        let physical = translate(pml4, virtual_address).ok_or("user pointer not mapped")?;
        let page_offset = (virtual_address & 0xFFF) as usize;
        let chunk = (PAGE_SIZE as usize - page_offset).min(source.len() - offset);
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr().add(offset),
                hhdm(physical) as *mut u8,
                chunk,
            );
        }
        offset += chunk;
    }
    Ok(())
}

pub fn validate_user_range(pml4: u64, address: u64, length: usize) -> bool {
    let mut offset = 0u64;
    while offset < length as u64 {
        let virtual_address = address + offset;
        if virtual_address >= USER_TOP || translate(pml4, virtual_address).is_none() {
            return false;
        }
        offset += PAGE_SIZE;
    }
    true
}

pub fn total_mapped_pages(_pml4: u64) -> usize {
    0
}

#[allow(dead_code)]
pub fn boot_hhdm() -> u64 {
    boot::info().hhdm_offset
}
