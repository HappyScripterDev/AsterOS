//! Ring-3 process creation: ELF loading into a private address space.

use crate::boot;
use crate::mm::{pmm, vmm};
use crate::sched;

const PF_X: u32 = 1;
const PF_W: u32 = 2;

struct UserImage {
    entry: u64,
    segments: alloc::vec::Vec<Segment>,
}

struct Segment {
    vaddr: u64,
    offset: u64,
    filesz: u64,
    memsz: u64,
    writable: bool,
    executable: bool,
}

fn rd16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn rd32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn rd64(bytes: &[u8], offset: usize) -> u64 {
    let mut value = [0u8; 8];
    value.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(value)
}

fn parse_elf(bytes: &[u8]) -> Result<UserImage, &'static str> {
    if bytes.len() < 64 || &bytes[0..4] != b"\x7FELF" {
        return Err("user image is not ELF");
    }
    if bytes[4] != 2 || bytes[5] != 1 || rd16(bytes, 18) != 62 {
        return Err("user image must be x86_64 ELF64");
    }
    let entry = rd64(bytes, 24);
    let phoff = rd64(bytes, 32) as usize;
    let phentsize = rd16(bytes, 54) as usize;
    let phnum = rd16(bytes, 56) as usize;
    if phentsize < 56 || phnum == 0 || phnum > 16 {
        return Err("unsupported program headers");
    }
    if phoff + phentsize * phnum > bytes.len() {
        return Err("program headers out of range");
    }
    let mut segments = alloc::vec::Vec::new();
    for index in 0..phnum {
        let ph = phoff + index * phentsize;
        if rd32(bytes, ph) != 1 {
            continue;
        }
        let flags = rd32(bytes, ph + 4);
        let offset = rd64(bytes, ph + 8);
        let vaddr = rd64(bytes, ph + 16);
        let filesz = rd64(bytes, ph + 32);
        let memsz = rd64(bytes, ph + 40);
        if filesz > memsz || offset + filesz > bytes.len() as u64 {
            return Err("invalid ELF segment sizes");
        }
        if vaddr & 0xFFF != 0 || memsz == 0 {
            return Err("ELF segment not page aligned");
        }
        if vaddr < vmm::USER_BASE || vaddr + memsz > vmm::USER_BASE + 0x0000_0000_1000_0000 {
            return Err("ELF segment outside the image window");
        }
        segments.push(Segment {
            vaddr,
            offset,
            filesz,
            memsz,
            writable: flags & PF_W != 0,
            executable: flags & PF_X != 0,
        });
    }
    if segments.is_empty() {
        return Err("ELF image has no loadable segments");
    }
    if entry < vmm::USER_BASE || entry >= vmm::USER_TOP {
        return Err("entry point outside user space");
    }
    Ok(UserImage { entry, segments })
}

unsafe fn map_segment(pml4: u64, bytes: &[u8], segment: &Segment) -> Result<(), &'static str> {
    let pages = ((segment.memsz + vmm::PAGE_SIZE - 1) / vmm::PAGE_SIZE) as usize;
    for page in 0..pages {
        let frame = pmm::alloc_zeroed_frame().ok_or("out of physical memory")?;
        let virtual_address = segment.vaddr + page as u64 * vmm::PAGE_SIZE;
        let mut flags = vmm::PTE_USER;
        if segment.writable {
            flags |= vmm::PTE_WRITE;
        }
        if !segment.executable {
            flags |= vmm::PTE_NX;
        }
        if let Err(error) = vmm::map_page(pml4, virtual_address, frame, flags) {
            pmm::free_frame(frame);
            return Err(error);
        }
        // Copy the file-backed part of this page.
        let page_start = page as u64 * vmm::PAGE_SIZE;
        if page_start < segment.filesz {
            let count = (segment.filesz - page_start).min(vmm::PAGE_SIZE) as usize;
            let source = bytes
                .as_ptr()
                .add(segment.offset as usize + page_start as usize);
            core::ptr::copy_nonoverlapping(source, vmm::hhdm(frame) as *mut u8, count);
        }
    }
    Ok(())
}

unsafe fn map_stack(pml4: u64) -> Result<u64, &'static str> {
    let pages = (sched::USER_STACK_SIZE / vmm::PAGE_SIZE) as usize;
    for page in 0..pages {
        let frame = pmm::alloc_zeroed_frame().ok_or("out of physical memory for stack")?;
        let virtual_address = vmm::USER_STACK_TOP - (page as u64 + 1) * vmm::PAGE_SIZE;
        if let Err(error) = vmm::map_page(
            pml4,
            virtual_address,
            frame,
            vmm::PTE_USER | vmm::PTE_WRITE | vmm::PTE_NX,
        ) {
            pmm::free_frame(frame);
            return Err(error);
        }
    }
    Ok(vmm::USER_STACK_TOP - 16)
}

/// Load a module provided by the bootloader and start it as a ring-3 process.
pub fn spawn_module(name: &str) -> Result<u64, &'static str> {
    let module = boot::module(name).ok_or("module not provided by the bootloader")?;
    let bytes = unsafe {
        core::slice::from_raw_parts(vmm::hhdm(module.address) as *const u8, module.size as usize)
    };
    let image = parse_elf(bytes)?;
    let pml4 = vmm::create_user_pml4().ok_or("out of memory for a page table")?;

    unsafe {
        for segment in &image.segments {
            map_segment(pml4, bytes, segment)?;
        }
        let stack_pointer = map_stack(pml4)?;
        let pid = sched::register_process(
            name,
            pml4,
            image.entry,
            stack_pointer,
            vmm::USER_BASE + 0x1000_0000,
        );
        Ok(pid)
    }
}
