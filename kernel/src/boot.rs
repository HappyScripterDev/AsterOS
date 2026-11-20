//! Boot information access and validation.

use core::sync::atomic::{AtomicU64, Ordering};

use aster_abi::{BootInfo, MemoryDescriptor, MemoryType, BOOTINFO_MAGIC, BOOTINFO_VERSION};

static BOOT_INFO: AtomicU64 = AtomicU64::new(0);

pub fn init(pointer: u64) -> &'static BootInfo {
    let info = unsafe { &*(pointer as *const BootInfo) };
    assert!(info.magic == BOOTINFO_MAGIC, "invalid BootInfo magic");
    assert!(info.version == BOOTINFO_VERSION, "unsupported BootInfo version");
    assert!(info.hhdm_offset != 0, "BootInfo has no direct map");
    assert!(info.memory_map.desc_size >= 40, "invalid UEFI memory map");
    BOOT_INFO.store(pointer, Ordering::Relaxed);
    info
}

pub fn info() -> &'static BootInfo {
    let pointer = BOOT_INFO.load(Ordering::Relaxed);
    assert!(pointer != 0, "BootInfo used before initialisation");
    unsafe { &*(pointer as *const BootInfo) }
}

pub fn physical_to_virtual(physical: u64) -> u64 {
    info().hhdm_offset + physical
}

pub fn descriptors() -> impl Iterator<Item = &'static MemoryDescriptor> {
    let info = info();
    let count = (info.memory_map.len / info.memory_map.desc_size) as usize;
    let base = (info.hhdm_offset + info.memory_map.ptr) as *const u8;
    (0..count).map(move |index| unsafe {
        &*(base.add(index * info.memory_map.desc_size as usize) as *const MemoryDescriptor)
    })
}

pub fn memory_type(descriptor: &MemoryDescriptor) -> MemoryType {
    match descriptor.ty {
        0 => MemoryType::Reserved,
        1 => MemoryType::LoaderCode,
        2 => MemoryType::LoaderData,
        3 => MemoryType::BootServicesCode,
        4 => MemoryType::BootServicesData,
        5 => MemoryType::RuntimeServicesCode,
        6 => MemoryType::RuntimeServicesData,
        7 => MemoryType::Conventional,
        8 => MemoryType::Unusable,
        9 => MemoryType::AcpiReclaim,
        10 => MemoryType::AcpiNvs,
        11 => MemoryType::Mmio,
        12 => MemoryType::MmioPortSpace,
        13 => MemoryType::PalCode,
        14 => MemoryType::Persistent,
        _ => MemoryType::Reserved,
    }
}

/// Highest address of usable RAM (excluding MMIO apertures).
pub fn usable_memory_end() -> u64 {
    let mut end = 0u64;
    for descriptor in descriptors() {
        if matches!(
            memory_type(descriptor),
            MemoryType::Conventional
                | MemoryType::LoaderCode
                | MemoryType::LoaderData
                | MemoryType::BootServicesCode
                | MemoryType::BootServicesData
        ) {
            end = end.max(descriptor.physical_start + descriptor.number_of_pages * 4096);
        }
    }
    end
}

pub fn module(name: &str) -> Option<&'static aster_abi::ModuleInfo> {
    let info = info();
    for index in 0..info.module_count as usize {
        let module = &info.modules[index];
        let len = module.name.iter().position(|&b| b == 0).unwrap_or(module.name.len());
        if len == name.len() && &module.name[..len] == name.as_bytes() {
            return Some(module);
        }
    }
    None
}
