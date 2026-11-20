//! Minimal ELF64 loader for the kernel image.

use crate::efi::{allocate_pages, BootServices};

const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;

#[derive(Clone, Copy, Default)]
pub struct Segment {
    pub vaddr: u64,
    pub phys: u64,
    pub memsz: u64,
    pub filesz: u64,
    pub writable: bool,
    pub executable: bool,
}

pub const MAX_SEGMENTS: usize = 8;

pub struct LoadedKernel {
    pub entry: u64,
    pub segments: [Segment; MAX_SEGMENTS],
    pub segment_count: usize,
    pub phys_start: u64,
    pub phys_end: u64,
}

fn rd16(bytes: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([bytes[off], bytes[off + 1]])
}

fn rd32(bytes: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn rd64(bytes: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        bytes[off],
        bytes[off + 1],
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
        bytes[off + 6],
        bytes[off + 7],
    ])
}

/// Load every PT_LOAD segment of an ELF64 image into freshly allocated
/// physical pages. Returns the entry point and the segment map.
pub unsafe fn load_kernel(bs: *mut BootServices, bytes: &[u8]) -> Result<LoadedKernel, &'static str> {
    if bytes.len() < 64 || &bytes[0..4] != b"\x7FELF" {
        return Err("kernel image is not an ELF file");
    }
    if bytes[4] != 2 || bytes[5] != 1 {
        return Err("kernel image is not 64-bit little-endian ELF");
    }
    if rd16(bytes, 18) != 62 {
        return Err("kernel image is not x86_64");
    }

    let entry = rd64(bytes, 24);
    let phoff = rd64(bytes, 32) as usize;
    let phentsize = rd16(bytes, 54) as usize;
    let phnum = rd16(bytes, 56) as usize;
    if phentsize < 56 || phnum == 0 || phnum > MAX_SEGMENTS {
        return Err("unsupported ELF program header count");
    }
    if phoff.checked_add(phentsize * phnum).map_or(true, |end| end > bytes.len()) {
        return Err("ELF program headers out of bounds");
    }

    let mut kernel = LoadedKernel {
        entry,
        segments: [Segment::default(); MAX_SEGMENTS],
        segment_count: 0,
        phys_start: u64::MAX,
        phys_end: 0,
    };

    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        if rd32(bytes, ph) != PT_LOAD {
            continue;
        }
        let flags = rd32(bytes, ph + 4);
        let offset = rd64(bytes, ph + 8) as usize;
        let vaddr = rd64(bytes, ph + 16);
        let filesz = rd64(bytes, ph + 32);
        let memsz = rd64(bytes, ph + 40);

        if filesz > memsz {
            return Err("ELF segment file size exceeds memory size");
        }
        if offset.checked_add(filesz as usize).map_or(true, |end| end > bytes.len()) {
            return Err("ELF segment data out of bounds");
        }
        if vaddr & 0xFFF != 0 || memsz == 0 {
            return Err("ELF segment is not page aligned");
        }

        let pages = ((memsz + 0xFFF) / 0x1000) as usize;
        let phys: u64 =
            allocate_pages(bs, pages).map_err(|_| "out of memory loading kernel")?;
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr().add(offset),
            phys as *mut u8,
            filesz as usize,
        );

        let seg = Segment {
            vaddr,
            phys,
            memsz,
            filesz,
            writable: flags & PF_W != 0,
            executable: flags & PF_X != 0,
        };
        kernel.segments[kernel.segment_count] = seg;
        kernel.segment_count += 1;
        kernel.phys_start = kernel.phys_start.min(phys);
        kernel.phys_end = kernel.phys_end.max(phys + pages as u64 * 0x1000);
    }

    if kernel.segment_count == 0 {
        return Err("kernel ELF has no loadable segments");
    }
    if kernel.entry < kernel.segments[0].vaddr {
        return Err("kernel entry point outside image");
    }
    Ok(kernel)
}
