//! Aster ABI: structures shared by the bootloader, kernel and user space.
//!
//! This crate must stay `no_std` and dependency free. Every `repr(C)` layout
//! here is part of the on-disk/boot contract; changing one requires bumping
//! [`BOOTINFO_VERSION`] or [`SYS_ABI_VERSION`].

#![no_std]

pub const BOOTINFO_MAGIC: u64 = 0x4153_5445_5230_3031; // "ASTER001"
pub const BOOTINFO_VERSION: u32 = 1;

/// Framebuffer pixel format as reported by UEFI GOP.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelFormat {
    Rgb = 0,
    Bgr = 1,
    Unknown = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FramebufferInfo {
    pub address: u64,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bpp: u32,
    pub format: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryMapInfo {
    /// Physical address of the UEFI memory descriptor array.
    pub ptr: u64,
    pub len: u64,
    pub desc_size: u64,
    pub desc_version: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleInfo {
    pub address: u64,
    pub size: u64,
    pub name: [u8; 24],
}

pub const MAX_MODULES: usize = 8;
pub const CMDLINE_LEN: usize = 128;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub flags: u32,
    /// Offset added to a physical address to obtain its direct-map address.
    pub hhdm_offset: u64,
    pub framebuffer: FramebufferInfo,
    pub memory_map: MemoryMapInfo,
    pub modules: [ModuleInfo; MAX_MODULES],
    pub module_count: u32,
    pub reserved: u32,
    pub rsdp_address: u64,
    pub boot_tsc: u64,
    /// Physical range covering the loaded kernel image.
    pub kernel_phys_start: u64,
    pub kernel_phys_size: u64,
    pub kernel_entry: u64,
    /// Physical address of the page table root the loader built.
    pub pml4_phys: u64,
    pub cmdline: [u8; CMDLINE_LEN],
    /// Physical address of this structure (identity == physical in UEFI).
    pub self_physical: u64,
}

impl BootInfo {
    pub const fn empty() -> Self {
        Self {
            magic: BOOTINFO_MAGIC,
            version: BOOTINFO_VERSION,
            flags: 0,
            hhdm_offset: 0,
            framebuffer: FramebufferInfo {
                address: 0,
                size: 0,
                width: 0,
                height: 0,
                stride: 0,
                bpp: 0,
                format: PixelFormat::Unknown as u32,
                reserved: 0,
            },
            memory_map: MemoryMapInfo {
                ptr: 0,
                len: 0,
                desc_size: 0,
                desc_version: 0,
                reserved: 0,
            },
            modules: [ModuleInfo {
                address: 0,
                size: 0,
                name: [0; 24],
            }; MAX_MODULES],
            module_count: 0,
            reserved: 0,
            rsdp_address: 0,
            boot_tsc: 0,
            kernel_phys_start: 0,
            kernel_phys_size: 0,
            kernel_entry: 0,
            pml4_phys: 0,
            cmdline: [0; CMDLINE_LEN],
            self_physical: 0,
        }
    }
}

/// UEFI memory descriptor types (spec 2.10, table 7.2).
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryType {
    Reserved = 0,
    LoaderCode = 1,
    LoaderData = 2,
    BootServicesCode = 3,
    BootServicesData = 4,
    RuntimeServicesCode = 5,
    RuntimeServicesData = 6,
    Conventional = 7,
    Unusable = 8,
    AcpiReclaim = 9,
    AcpiNvs = 10,
    Mmio = 11,
    MmioPortSpace = 12,
    PalCode = 13,
    Persistent = 14,
    Unaccepted = 15,
    Max = 16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryDescriptor {
    pub ty: u32,
    pub pad: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub number_of_pages: u64,
    pub attribute: u64,
}

/// System call numbers. The ABI is versioned as a whole; individual calls may
/// be added but never renumbered.
pub mod sys {
    pub const ABI_VERSION: u64 = 1;

    pub const EXIT: u64 = 1;
    pub const WRITE: u64 = 2;
    pub const GETPID: u64 = 3;
    pub const YIELD: u64 = 4;
    pub const SLEEP_MS: u64 = 5;
    pub const TIME_MS: u64 = 6;
    pub const FB_INFO: u64 = 7;
    pub const FB_MAP: u64 = 8;
    pub const INPUT_POLL: u64 = 9;
    pub const VM_ALLOC: u64 = 10;
    pub const LOG: u64 = 11;
    pub const OPEN: u64 = 12;
    pub const READ: u64 = 13;
    pub const WRITE_FD: u64 = 14;
    pub const CLOSE: u64 = 15;
    pub const SEEK: u64 = 16;
    pub const READDIR: u64 = 17;
    pub const STAT: u64 = 18;
    pub const UNLINK: u64 = 19;
    pub const MKDIR: u64 = 20;
    pub const SPAWN: u64 = 21;
    pub const WAIT: u64 = 22;
    pub const SYSTEM_INFO: u64 = 23;
    pub const CHANNEL_CREATE: u64 = 24;
    pub const CHANNEL_SEND: u64 = 25;
    pub const CHANNEL_RECV: u64 = 26;
    pub const SET_THEME: u64 = 27;
    pub const SHUTDOWN: u64 = 28;
    pub const RTC: u64 = 29;
    pub const PROC_LIST: u64 = 30;
    pub const STORAGE_INFO: u64 = 31;
    pub const CHANNEL_CLOSE: u64 = 32;
}

/// Error codes returned in `rax` by system calls. Negative values are errors.
pub mod errno {
    pub const OK: i64 = 0;
    pub const EPERM: i64 = -1;
    pub const ENOENT: i64 = -2;
    pub const EBADF: i64 = -3;
    pub const ENOMEM: i64 = -4;
    pub const EINVAL: i64 = -5;
    pub const EIO: i64 = -6;
    pub const EEXIST: i64 = -7;
    pub const ENOTDIR: i64 = -8;
    pub const EISDIR: i64 = -9;
    pub const ENOSPC: i64 = -10;
    pub const ENAMETOOLONG: i64 = -11;
    pub const ECHILD: i64 = -12;
    pub const ESRCH: i64 = -13;
    pub const EPIPE: i64 = -14;
    pub const ETIMEDOUT: i64 = -15;
    pub const ENOSYS: i64 = -38;
}

/// Input event kinds delivered to the desktop.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputKind {
    Key = 1,
    MouseMove = 2,
    MouseButton = 3,
    MouseWheel = 4,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputEvent {
    pub kind: u32,
    /// Key: ASCII for printable keys, `keys::*` for special keys.
    /// Button: 0 left, 1 right, 2 middle.
    pub code: u32,
    /// Key/button: 1 press, 0 release.
    pub value: i32,
    /// Absolute pointer position in framebuffer pixels.
    pub x: i32,
    pub y: i32,
    /// Mouse button bitmask after the event.
    pub buttons: u32,
    /// Modifier state (see `mods`) for key events.
    pub mods: u32,
}

/// Modifier bits carried by [`InputEvent::mods`].
pub mod mods {
    pub const SHIFT: u32 = 1;
    pub const CTRL: u32 = 2;
    pub const ALT: u32 = 4;
    pub const META: u32 = 8;
    pub const CAPS: u32 = 16;
}

/// Stable key codes used by the kernel input driver and the desktop.
/// Printable characters are delivered as their ASCII value instead.
pub mod keys {
    pub const ESC: u32 = 0x100;
    pub const BACKSPACE: u32 = 0x101;
    pub const TAB: u32 = 0x102;
    pub const ENTER: u32 = 0x103;
    pub const DELETE: u32 = 0x104;
    pub const HOME: u32 = 0x105;
    pub const END: u32 = 0x106;
    pub const PAGE_UP: u32 = 0x107;
    pub const PAGE_DOWN: u32 = 0x108;
    pub const LEFT: u32 = 0x109;
    pub const RIGHT: u32 = 0x10A;
    pub const UP: u32 = 0x10B;
    pub const DOWN: u32 = 0x10C;
    pub const INSERT: u32 = 0x10D;
    pub const F1: u32 = 0x110;
    pub const F2: u32 = 0x111;
    pub const F3: u32 = 0x112;
    pub const F4: u32 = 0x113;
    pub const F5: u32 = 0x114;
    pub const F6: u32 = 0x115;
    pub const F7: u32 = 0x116;
    pub const F8: u32 = 0x117;
    pub const F9: u32 = 0x118;
    pub const F10: u32 = 0x119;
    pub const F11: u32 = 0x11A;
    pub const F12: u32 = 0x11B;
    pub const LEFT_SHIFT: u32 = 0x120;
    pub const RIGHT_SHIFT: u32 = 0x121;
    pub const LEFT_CTRL: u32 = 0x122;
    pub const RIGHT_CTRL: u32 = 0x123;
    pub const LEFT_ALT: u32 = 0x124;
    pub const RIGHT_ALT: u32 = 0x125;
    pub const CAPS_LOCK: u32 = 0x126;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SystemInfo {
    pub uptime_ms: u64,
    pub total_memory_kib: u64,
    pub free_memory_kib: u64,
    pub kernel_heap_used_kib: u64,
    pub thread_count: u32,
    pub process_count: u32,
    pub cpu_count: u32,
    pub timer_hz: u32,
    pub context_switches: u64,
    pub ticks: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FileStat {
    pub size: u64,
    pub is_dir: u32,
    pub mode: u32,
    pub inode: u64,
}

pub mod open_flags {
    pub const READ: u32 = 1;
    pub const WRITE: u32 = 2;
    pub const CREATE: u32 = 4;
    pub const TRUNC: u32 = 8;
    pub const DIRECTORY: u32 = 16;
}

pub const DIRENT_NAME_LEN: usize = 64;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DirEntry {
    pub inode: u64,
    pub ty: u32,
    pub name_len: u32,
    pub name: [u8; DIRENT_NAME_LEN],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RtcTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub weekday: u8,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcessInfo {
    pub pid: u64,
    pub threads: u32,
    pub state: u32,
    pub name: [u8; 24],
}

/// Capacity of the persistent home volume. `mounted == 0` means the system
/// booted without a data disk and is running on the in-memory root only.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StorageInfo {
    pub mounted: u32,
    pub sector_bytes: u32,
    pub cluster_bytes: u32,
    pub reserved: u32,
    pub total_bytes: u64,
    pub free_bytes: u64,
}
