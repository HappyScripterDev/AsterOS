//! Aster Boot - the UEFI stage of Aster OS.
//!
//! Responsibilities:
//!   1. initialise diagnostics (COM1 + firmware console),
//!   2. locate the ESP and load KERNEL.ELF and the desktop module,
//!   3. load ELF PT_LOAD segments into physical memory,
//!   4. build the higher-half page tables and the direct physical map,
//!   5. create the BootInfo structure for the kernel,
//!   6. ExitBootServices and transfer control to the kernel.
//!
//! No Linux boot protocol, GRUB, or firmware runtime services are used after
//! ExitBootServices. Failure is always fatal and reported, never silent.

#![no_std]
#![no_main]

mod efi;
mod elf;
mod paging;

use core::fmt::{self, Write};
use core::mem::size_of;
use core::ptr;

use aster_abi::{
    BootInfo, FramebufferInfo, MemoryMapInfo, ModuleInfo, PixelFormat, BOOTINFO_MAGIC,
    BOOTINFO_VERSION, MAX_MODULES,
};
use efi::*;
use paging::{PageTables, HHDM_OFFSET};

const COM1: u16 = 0x3F8;
const VERSION: &str = "0.1.0";
const KERNEL_PATH: &str = "\\KERNEL.ELF";
const DESK_PATH: &str = "\\DESK.ELF";
const TRAMPOLINE: [u8; 8] = [0x48, 0x0F, 0x22, 0xDE, 0x48, 0xFF, 0xE0, 0x90];

#[inline]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

unsafe fn serial_init() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x01);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x03);
}

unsafe fn serial_write_raw(s: &str) {
    for &byte in s.as_bytes() {
        let mut spins = 0u32;
        while inb(COM1 + 5) & 0x20 == 0 {
            spins += 1;
            if spins > 200_000 {
                break;
            }
        }
        outb(COM1, byte);
    }
}

unsafe fn serial_write(s: &str) {
    for &byte in s.as_bytes() {
        if byte == b'\n' {
            serial_write_raw("\r\n");
        } else {
            let b = [byte];
            if let Ok(chunk) = core::str::from_utf8(&b) {
                serial_write_raw(chunk);
            }
        }
    }
}

static mut CONSOLE: *mut SimpleTextOutput = ptr::null_mut();

unsafe fn console_write(s: &str) {
    let out = CONSOLE;
    if out.is_null() {
        return;
    }
    let mut buffer = [0u16; 256];
    let mut n = 0usize;
    let flush = |buffer: &[u16], n: usize| {
        if n > 0 {
            let mut local = [0u16; 257];
            local[..n].copy_from_slice(&buffer[..n]);
            local[n] = 0;
            let _ = ((*out).output_string)(out, local.as_ptr());
        }
    };
    for &byte in s.as_bytes() {
        if byte == b'\n' {
            if n >= 254 {
                flush(&buffer, n);
                n = 0;
            }
            buffer[n] = b'\r' as u16;
            buffer[n + 1] = b'\n' as u16;
            n += 2;
        } else {
            if n >= 255 {
                flush(&buffer, n);
                n = 0;
            }
            buffer[n] = byte as u16;
            n += 1;
        }
    }
    flush(&buffer, n);
}

pub fn klog(args: fmt::Arguments) {
    struct SerialWriter;
    impl Write for SerialWriter {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            unsafe { serial_write(s) };
            Ok(())
        }
    }
    let _ = SerialWriter.write_fmt(args);
}

macro_rules! log {
    ($($arg:tt)*) => { $crate::klog(format_args!($($arg)*)) };
}

fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

fn fail(message: &str, status: Status) -> ! {
    log!("\n[fatal] {}\n[fatal] UEFI status = {:#x}\n", message, status);
    halt()
}

fn fail_str(message: &str) -> ! {
    log!("\n[fatal] {}\n", message);
    halt()
}

unsafe fn u16_name(path: &str) -> [u16; 64] {
    let mut out = [0u16; 64];
    let bytes = path.as_bytes();
    let n = bytes.len().min(63);
    for i in 0..n {
        out[i] = bytes[i] as u16;
    }
    out
}

unsafe fn file_size(file: *mut FileProtocol) -> Result<u64, Status> {
    let mut info = [0u8; 1024];
    let mut size = info.len();
    let status = ((*file).get_info)(
        file,
        &FILE_INFO_GUID,
        &mut size,
        info.as_mut_ptr(),
    );
    if status != EFI_SUCCESS {
        return Err(status);
    }
    // EFI_FILE_INFO: Size (u64), FileSize (u64), ...
    Ok(u64::from_le_bytes([
        info[8], info[9], info[10], info[11], info[12], info[13], info[14], info[15],
    ]))
}

/// Open a file relative to `root` for reading. UEFI firmware differs in
/// whether it accepts a leading separator, so try the documented path first
/// and then fall back to equivalent forms.
#[inline(never)]
unsafe fn file_open(
    root: *mut FileProtocol,
    path: &str,
) -> Result<*mut FileProtocol, Status> {
    let stripped = path.strip_prefix('\\').unwrap_or(path);
    let variants = [path, stripped];
    let mut last = EFI_SUCCESS;
    for variant in variants {
        let name = u16_name(variant);
        let mut file: *mut FileProtocol = ptr::null_mut();
        let status = ((*root).open)(root, &mut file, name.as_ptr(), EFI_FILE_MODE_READ, 0);
        if status == EFI_SUCCESS && !file.is_null() {
            return Ok(file);
        }
        last = status;
        log!("  open '{}' -> {:#x}\n", variant, status);
    }
    Err(last)
}

unsafe fn read_file(bs: *mut BootServices, root: *mut FileProtocol, path: &str) -> Result<(&'static [u8], u64), &'static str> {
    let file = match file_open(root, path) {
        Ok(file) => file,
        Err(status) => {
            log!("[err] open '{}' failed: status {:#x}\n", path, status);
            return Err("cannot open kernel file");
        }
    };
    let size = match file_size(file) {
        Ok(size) => size,
        Err(status) => {
            log!("[err] stat '{}' failed: status {:#x}\n", path, status);
            return Err("cannot stat kernel file");
        }
    };
    log!("  kernel image: {} bytes\n", size);
    if size == 0 || size > 64 * 1024 * 1024 {
        return Err("implausible kernel file size");
    }
    let buffer = allocate_pool(bs, size as usize).map_err(|_| "out of memory for kernel file")?;
    let slice = core::slice::from_raw_parts_mut(buffer, size as usize);
    (*file).read_exact(slice).map_err(|_| "cannot read kernel file")?;
    let _ = ((*file).close)(file);
    Ok((core::slice::from_raw_parts(buffer, size as usize), size))
}

unsafe fn load_module(
    bs: *mut BootServices,
    root: *mut FileProtocol,
    path: &str,
    name: &str,
) -> Result<ModuleInfo, &'static str> {
    let u16path = u16_name(path);
    let _ = u16path;
    let file = match file_open(root, path) {
        Ok(file) => file,
        Err(status) => {
            log!("[err] open module '{}' failed: {:#x}\n", path, status);
            return Err("cannot open desktop module");
        }
    };
    let size = file_size(file).map_err(|_| "cannot stat desktop module")?;
    if size == 0 || size > 256 * 1024 * 1024 {
        return Err("implausible desktop module size");
    }
    let pages = ((size + 0xFFF) / 0x1000) as usize;
    let address = allocate_pages(bs, pages).map_err(|_| "out of memory for desktop module")?;
    let buffer = core::slice::from_raw_parts_mut(address as *mut u8, pages * 0x1000);
    let file_slice = &mut buffer[..size as usize];
    let read_result = {
        let mut offset = 0usize;
        let mut error = EFI_SUCCESS;
        while offset < file_slice.len() {
            let mut chunk = file_slice.len() - offset;
            let status = ((*file).read)(file, &mut chunk, file_slice.as_mut_ptr().add(offset));
            if status != EFI_SUCCESS {
                error = status;
                break;
            }
            if chunk == 0 {
                break;
            }
            offset += chunk;
        }
        error
    };
    let _ = ((*file).close)(file);
    if read_result != EFI_SUCCESS {
        return Err("cannot read desktop module");
    }
    let mut module = ModuleInfo {
        address,
        size,
        name: [0; 24],
    };
    for (i, b) in name.as_bytes().iter().take(23).enumerate() {
        module.name[i] = *b;
    }
    log!("  module '{}': {} bytes at {:#x}\n", name, size, address);
    Ok(module)
}

unsafe fn get_memory_map(bs: *mut BootServices) -> Result<(&'static mut [u8], usize, usize, u32), &'static str> {
    let mut size = 0usize;
    let mut key = 0usize;
    let mut descriptor_size = 0usize;
    let mut descriptor_version = 0u32;
    let _ = ((*bs).get_memory_map)(
        &mut size,
        ptr::null_mut(),
        &mut key,
        &mut descriptor_size,
        &mut descriptor_version,
    );
    if size == 0 || descriptor_size == 0 {
        return Err("firmware returned an empty memory map");
    }
    size += descriptor_size * 4;
    let buffer = allocate_pool(bs, size).map_err(|_| "out of memory for the memory map")?;
    let status = ((*bs).get_memory_map)(
        &mut size,
        buffer,
        &mut key,
        &mut descriptor_size,
        &mut descriptor_version,
    );
    if status != EFI_SUCCESS {
        return Err("cannot read the firmware memory map");
    }
    Ok((
        core::slice::from_raw_parts_mut(buffer, size),
        key,
        descriptor_size,
        descriptor_version,
    ))
}

unsafe fn find_rsdp(st: *mut SystemTable) -> u64 {
    let count = (*st).number_of_table_entries;
    let table = (*st).configuration_table;
    if table.is_null() {
        return 0;
    }
    for i in 0..count {
        let entry = &*table.add(i);
        if entry.vendor_guid == ACPI2_TABLE_GUID {
            return entry.vendor_table as u64;
        }
    }
    0
}

unsafe fn choose_mode(gop: *mut GraphicsOutput, console_out: *mut SimpleTextOutput) {
    let mode = (*gop).mode;
    if mode.is_null() {
        return;
    }
    let max = (*mode).max_mode;
    let mut best = (*mode).mode;
    let mut best_score = 0u32;
    for candidate in 0..max {
        let mut info_size = 0usize;
        let mut info: *mut GopModeInfo = ptr::null_mut();
        let status = ((*gop).query_mode)(gop, candidate, &mut info_size, &mut info);
        if status != EFI_SUCCESS || info.is_null() {
            continue;
        }
        let (w, h) = ((*info).horizontal_resolution, (*info).vertical_resolution);
        let score = match (w, h) {
            (1280, 800) => 100,
            (1440, 900) => 90,
            (1280, 720) => 80,
            (1024, 768) => 70,
            (1920, 1080) => 60,
            _ => 10,
        };
        if score > best_score {
            best_score = score;
            best = candidate;
        }
    }
    if best_score >= 70 {
        let status = ((*gop).set_mode)(gop, best);
        if status == EFI_SUCCESS {
            log!("  GOP mode set to index {}\n", best);
        }
    }
    let _ = console_out;
}

unsafe fn conout(st: *mut SystemTable) -> *mut SimpleTextOutput {
    (*st).con_out
}

#[no_mangle]
pub extern "efiapi" fn efi_main(image: Handle, system_table: *mut SystemTable) -> Status {
    unsafe {
        serial_init();
        CONSOLE = conout(system_table);
        log!("\n==============================================\n");
        log!(" Aster Boot {} - UEFI loader\n", VERSION);
        log!(" (c) Aster OS project - independent OS\n");
        log!("==============================================\n");
        log!("BOOT-LOADER-START\n");
    }

    let bs = unsafe { (*system_table).boot_services };
    if bs.is_null() {
        fail_str("firmware did not provide boot services");
    }

    unsafe {
        let _ = ((*bs).set_watchdog_timer)(0, 0, 0, ptr::null_mut());
    }

    // --- firmware protocols -------------------------------------------------
    let loaded_image = unsafe {
        open_protocol::<LoadedImage>(bs, image, &LOADED_IMAGE_GUID)
            .unwrap_or_else(|s| fail("cannot open LoadedImage protocol", s))
    };
    let sfs = unsafe {
        open_protocol::<SimpleFileSystem>(
            bs,
            (*loaded_image).device_handle,
            &SIMPLE_FILE_SYSTEM_GUID,
        )
        .unwrap_or_else(|s| fail("cannot open the EFI System Partition", s))
    };
    let mut root: *mut FileProtocol = ptr::null_mut();
    unsafe {
        let status = ((*sfs).open_volume)(sfs, &mut root);
        if status != EFI_SUCCESS || root.is_null() {
            fail("cannot open the ESP root directory", status);
        }
    }
    log!("[ok] EFI System Partition opened\n");
    unsafe {
    }

    // --- graphics ----------------------------------------------------------
    let mut framebuffer = FramebufferInfo {
        address: 0,
        size: 0,
        width: 0,
        height: 0,
        stride: 0,
        bpp: 32,
        format: PixelFormat::Unknown as u32,
        reserved: 0,
    };
    if let Ok(gop) = unsafe { locate_protocol::<GraphicsOutput>(bs, &GRAPHICS_OUTPUT_GUID) } {
        unsafe {
            choose_mode(gop, conout(system_table));
            let mode = (*gop).mode;
            if !mode.is_null() {
                let info = (*mode).info;
                if !info.is_null() {
                    framebuffer.address = (*mode).frame_buffer_base;
                    framebuffer.size = (*mode).frame_buffer_size as u64;
                    framebuffer.width = (*info).horizontal_resolution;
                    framebuffer.height = (*info).vertical_resolution;
                    framebuffer.stride = (*info).pixels_per_scan_line;
                    framebuffer.format = if (*info).pixel_format == 1 {
                        PixelFormat::Bgr as u32
                    } else {
                        PixelFormat::Rgb as u32
                    };
                }
            }
        }
        log!(
            "[ok] framebuffer {}x{} stride={} at {:#x} ({} MiB)\n",
            framebuffer.width,
            framebuffer.height,
            framebuffer.stride,
            framebuffer.address,
            framebuffer.size / (1024 * 1024)
        );
    } else {
        log!("[warn] GOP unavailable: graphics will be disabled\n");
    }

    // --- kernel and modules -------------------------------------------------
    let (kernel_bytes, _kernel_size) = unsafe {
        read_file(bs, root, KERNEL_PATH)
            .unwrap_or_else(|e| fail_str(e))
    };
    let kernel = unsafe {
        elf::load_kernel(bs, kernel_bytes).unwrap_or_else(|e| fail_str(e))
    };
    log!(
        "[ok] kernel ELF loaded: {} segments, entry {:#x}\n",
        kernel.segment_count,
        kernel.entry
    );

    let mut modules = [ModuleInfo { address: 0, size: 0, name: [0; 24] }; MAX_MODULES];
    let mut module_count = 0usize;
    match unsafe { load_module(bs, root, DESK_PATH, "desk") } {
        Ok(module) => {
            modules[0] = module;
            module_count = 1;
        }
        Err(e) => log!("[warn] desktop module not available: {}\n", e),
    }

    // --- page tables --------------------------------------------------------
    let mut tables = unsafe {
        PageTables::new(bs).unwrap_or_else(|e| fail_str(e))
    };

    // Determine the physical address that must be mapped.
    let (map_slice, map_key, map_desc_size, map_version) = unsafe {
        get_memory_map(bs).unwrap_or_else(|e| fail_str(e))
    };
    let mut max_phys = framebuffer.address.saturating_add(framebuffer.size);
    let count = map_slice.len() / map_desc_size;
    for i in 0..count {
        unsafe {
            let desc = &*(map_slice.as_ptr().add(i * map_desc_size) as *const MemoryDescriptor);
            // Map usable RAM and firmware/loader regions. MMIO apertures are
            // mapped on demand when a driver claims the device.
            let usable = matches!(desc.ty, 1..=4 | 7 | 9 | 10 | 14);
            if usable {
                let end = desc.physical_start.saturating_add(desc.number_of_pages * 0x1000);
                max_phys = max_phys.max(end);
            }
        }
    }
    max_phys = (max_phys + 0x1F_FFFF) & !0x1F_FFFF;
    if max_phys > 64 * 1024 * 1024 * 1024 {
        log!("[warn] physical address space above 64 GiB; clamping direct map\n");
        max_phys = 64 * 1024 * 1024 * 1024;
    }
    log!("[..] building page tables for {} GiB of physical address space\n", max_phys / (1024 * 1024 * 1024));

    unsafe {
        tables
            .map_physical(max_phys)
            .unwrap_or_else(|e| fail_str(e));
        tables
            .map_kernel(&kernel.segments[..kernel.segment_count])
            .unwrap_or_else(|e| fail_str(e));
    }
    let trampoline = unsafe {
        allocate_pages(bs, 1).unwrap_or_else(|s| fail("cannot allocate the trampoline page", s))
    };
    unsafe {
        ptr::copy_nonoverlapping(TRAMPOLINE.as_ptr(), trampoline as *mut u8, TRAMPOLINE.len());
        tables
            .map_page(trampoline, trampoline, paging::PTE_GLOBAL)
            .unwrap_or_else(|e| fail_str(e));
    }
    log!("[ok] kernel image mapped at {:#x}, direct map at {:#x}\n", paging::KERNEL_BASE, HHDM_OFFSET);

    // --- BootInfo -----------------------------------------------------------
    let bootinfo_phys = unsafe {
        allocate_pool(bs, size_of::<BootInfo>())
            .unwrap_or_else(|s| fail("cannot allocate BootInfo", s)) as u64
    };
    unsafe {
        ptr::write(bootinfo_phys as *mut BootInfo, BootInfo::empty());
        let info = &mut *(bootinfo_phys as *mut BootInfo);
        info.magic = BOOTINFO_MAGIC;
        info.version = BOOTINFO_VERSION;
        info.hhdm_offset = HHDM_OFFSET;
        info.framebuffer = framebuffer;
        info.boot_tsc = core::arch::x86_64::_rdtsc();
        info.kernel_phys_start = kernel.phys_start;
        info.kernel_phys_size = kernel.phys_end - kernel.phys_start;
        info.kernel_entry = kernel.entry;
        info.pml4_phys = tables.pml4;
        info.rsdp_address = find_rsdp(system_table);
        info.module_count = module_count as u32;
        info.modules = modules;
        info.self_physical = bootinfo_phys;
        let cmdline = b"root=ramfs console=serial video=gop";
        info.cmdline[..cmdline.len()].copy_from_slice(cmdline);
    }

    if unsafe { (*root).close as usize } == 0 {
        // unreachable; keeps the compiler from assuming root escapes
    }

    // --- exit boot services and enter the kernel ---------------------------
    let mut attempts = 0;
    loop {
        let (map, key, desc_size, desc_version) =
            unsafe { get_memory_map(bs).unwrap_or_else(|e| fail_str(e)) };
        unsafe {
            let info = &mut *(bootinfo_phys as *mut BootInfo);
            info.memory_map = MemoryMapInfo {
                ptr: map.as_ptr() as u64,
                len: map.len() as u64,
                desc_size: desc_size as u64,
                desc_version,
                reserved: 0,
            };
        }
        let status = unsafe { ((*bs).exit_boot_services)(image, key) };
        if status == EFI_SUCCESS {
            break;
        }
        attempts += 1;
        if attempts > 4 {
            fail("ExitBootServices failed after retries", status);
        }
        log!("[warn] ExitBootServices retry {} (status {:#x})\n", attempts, status);
    }
    let _ = (map_key, map_version);

    // No boot services from this point on.
    unsafe {
        log!("\n[ok] ExitBootServices complete\n");
        log!("[..] transferring control to the kernel at {:#x}\n", kernel.entry);
        log!("BOOT-HANDOFF-OK\n");
        enter_kernel(
            kernel.entry,
            tables.pml4,
            HHDM_OFFSET + bootinfo_phys,
            trampoline,
        );
    }
}

unsafe fn enter_kernel(entry: u64, pml4: u64, bootinfo: u64, trampoline: u64) -> ! {
    core::arch::asm!(
        "jmp {trampoline}",
        trampoline = in(reg) trampoline,
        in("rax") entry,
        in("rsi") pml4,
        in("rdi") bootinfo,
        options(noreturn)
    );
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe {
        serial_write("\n[fatal] bootloader panic\n");
        let mut writer = SerialPanic;
        let _ = write!(writer, "{}\n", info);
    }
    halt()
}

struct SerialPanic;
impl Write for SerialPanic {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe { serial_write(s) };
        Ok(())
    }
}
