//! PCI configuration-space access (mechanism #1).
//!
//! The system currently needs one device: the q35 SATA/AHCI controller. The
//! scan is deliberately small and deterministic (bus 0, functions in order).

use core::arch::asm;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

const CLASS_STORAGE: u8 = 0x01;
const SUBCLASS_SATA: u8 = 0x06;
const PROG_IF_AHCI: u8 = 0x01;

#[inline]
fn outl(port: u16, value: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags));
    }
}

#[inline]
fn inl(port: u16) -> u32 {
    let value: u32;
    unsafe {
        asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}

#[derive(Clone, Copy)]
pub struct Device {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor: u16,
    pub id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
}

fn address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | (offset as u32 & 0xFC)
}

pub fn read32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    outl(CONFIG_ADDRESS, address(bus, device, function, offset));
    inl(CONFIG_DATA)
}

pub fn write32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    outl(CONFIG_ADDRESS, address(bus, device, function, offset));
    outl(CONFIG_DATA, value);
}

fn read16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let value = read32(bus, device, function, offset);
    ((value >> ((offset as u32 & 2) * 8)) & 0xFFFF) as u16
}

fn write16(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let shift = (offset as u32 & 2) * 8;
    let mut current = read32(bus, device, function, offset);
    current &= !(0xFFFF << shift);
    current |= (value as u32) << shift;
    write32(bus, device, function, offset, current);
}

/// Locate a device by vendor and device id on bus 0.
pub fn find_device(vendor_id: u16, device_id: u16) -> Option<Device> {
    for device in 0..32u8 {
        for function in 0..8u8 {
            let vendor = read32(0, device, function, 0) as u16;
            if vendor == 0xFFFF {
                if function == 0 {
                    break;
                }
                continue;
            }
            let id = (read32(0, device, function, 0) >> 16) as u16;
            if vendor == vendor_id && id == device_id {
                let class = (read32(0, device, function, 0x08) >> 24) as u8;
                let subclass = (read32(0, device, function, 0x08) >> 16) as u8;
                let prog_if = (read32(0, device, function, 0x08) >> 8) as u8;
                return Some(Device {
                    bus: 0,
                    device,
                    function,
                    vendor,
                    id,
                    class,
                    subclass,
                    prog_if,
                });
            }
        }
    }
    None
}

/// Locate the first AHCI controller on bus 0.
pub fn find_ahci() -> Option<Device> {
    for device in 0..32u8 {
        for function in 0..8u8 {
            let vendor = read32(0, device, function, 0) as u16;
            if vendor == 0xFFFF {
                if function == 0 {
                    break;
                }
                continue;
            }
            let class = (read32(0, device, function, 0x08) >> 24) as u8;
            let subclass = (read32(0, device, function, 0x08) >> 16) as u8;
            let prog_if = (read32(0, device, function, 0x08) >> 8) as u8;
            if class == CLASS_STORAGE && subclass == SUBCLASS_SATA && prog_if == PROG_IF_AHCI {
                let id = (read32(0, device, function, 0) >> 16) as u16;
                return Some(Device {
                    bus: 0,
                    device,
                    function,
                    vendor,
                    id,
                    class,
                    subclass,
                    prog_if,
                });
            }
        }
    }
    None
}

/// Read a memory BAR (index 0..5, in 4-byte configuration slots).
pub fn memory_bar(device: &Device, index: u8) -> Option<u64> {
    let offset = 0x10 + index * 4;
    let low = read32(device.bus, device.device, device.function, offset);
    if low & 1 != 0 {
        return None; // I/O space BAR
    }
    let base = (low & !0xF) as u64;
    let kind = (low >> 1) & 3;
    if kind == 2 && index < 5 {
        let high = read32(device.bus, device.device, device.function, offset + 4) as u64;
        return Some((high << 32) | base);
    }
    Some(base)
}

/// Enable memory decoding and bus mastering so the controller can DMA.
pub fn enable_device(device: &Device) {
    let mut command = read16(device.bus, device.device, device.function, 0x04);
    command |= (1 << 1) | (1 << 2);
    write16(device.bus, device.device, device.function, 0x04, command);
}

pub fn describe(device: &Device) -> (u16, u16) {
    (device.vendor, device.id)
}

/// Walk the PCI capability list for a capability id.
pub fn find_capability(device: &Device, id: u8) -> Option<u8> {
    let status = read16(device.bus, device.device, device.function, 0x06);
    if status & (1 << 4) == 0 {
        return None;
    }
    let mut pointer = (read16(device.bus, device.device, device.function, 0x34) & 0xFFFC) as u8;
    for _ in 0..32 {
        if pointer == 0 {
            break;
        }
        let header = read32(device.bus, device.device, device.function, pointer);
        if (header & 0xFF) as u8 == id {
            return Some(pointer);
        }
        pointer = ((header >> 8) & 0xFC) as u8;
    }
    None
}

/// Enable one MSI vector on a device. Returns false when the device has no MSI
/// capability; the caller then keeps polling.
pub fn enable_msi(device: &Device, vector: u8) -> bool {
    let Some(capability) = find_capability(device, 0x05) else {
        return false;
    };
    let control = read16(device.bus, device.device, device.function, capability + 2);
    let address = 0xFEE0_0000u32 | ((crate::arch::apic::id() as u32) << 12);
    write32(
        device.bus,
        device.device,
        device.function,
        capability + 4,
        address,
    );
    if control & (1 << 7) != 0 {
        write32(
            device.bus,
            device.device,
            device.function,
            capability + 8,
            0,
        );
        write16(
            device.bus,
            device.device,
            device.function,
            capability + 12,
            vector as u16,
        );
    } else {
        write16(
            device.bus,
            device.device,
            device.function,
            capability + 8,
            vector as u16,
        );
    }
    let mut enabled = read16(device.bus, device.device, device.function, capability + 2);
    enabled &= !(0x7 << 4);
    enabled |= 1;
    write16(
        device.bus,
        device.device,
        device.function,
        capability + 2,
        enabled,
    );
    true
}

/// PCI interrupt line from config space, or 0 when the device uses no INTx.
pub fn interrupt_line(device: &Device) -> u8 {
    (read32(device.bus, device.device, device.function, 0x3C) & 0xFF) as u8
}

/// Enumerate bus 0 and fill `out`. Returns the number of present functions.
pub fn enumerate(out: &mut [aster_abi::DeviceInfo]) -> usize {
    let mut count = 0usize;
    for device in 0..32u8 {
        for function in 0..8u8 {
            let vendor = read32(0, device, function, 0) as u16;
            if vendor == 0xFFFF {
                continue;
            }
            if count >= out.len() {
                return count;
            }
            let header = read32(0, device, function, 0x08);
            let class = ((header >> 24) & 0xFF) as u8;
            let subclass = ((header >> 16) & 0xFF) as u8;
            let prog_if = ((header >> 8) & 0xFF) as u8;
            let irq = (read32(0, device, function, 0x3C) & 0xFF) as u8;
            let device_id = (read32(0, device, function, 0) >> 16) as u16;
            let driver = (vendor == 0x8086 && device_id == 0x100E)
                || (class == CLASS_STORAGE && subclass == SUBCLASS_SATA && prog_if == PROG_IF_AHCI);
            out[count] = aster_abi::DeviceInfo {
                bus: 0,
                device,
                function,
                class,
                subclass,
                prog_if,
                irq,
                flags: if driver { 1 } else { 0 },
                vendor,
                device_id,
                reserved: 0,
            };
            count += 1;
            // A multi-function device may have more than one function.
            if function == 0 && read32(0, device, 0, 0x0C) & (1 << 23) == 0 {
                break;
            }
        }
    }
    count
}

/// Locate the xHCI controller by PCI class 0x0C/0x03/0x30.
pub fn find_xhci() -> Option<Device> {
    for device in 0..32u8 {
        for function in 0..8u8 {
            let vendor = read32(0, device, function, 0) as u16;
            if vendor == 0xFFFF {
                if function == 0 {
                    break;
                }
                continue;
            }
            let class = (read32(0, device, function, 0x08) >> 24) as u8;
            let subclass = (read32(0, device, function, 0x08) >> 16) as u8;
            let prog_if = (read32(0, device, function, 0x08) >> 8) as u8;
            if class == 0x0C && subclass == 0x03 && prog_if == 0x30 {
                return Some(Device {
                    bus: 0,
                    device,
                    function,
                    vendor,
                    id: (read32(0, device, function, 0) >> 16) as u16,
                    class,
                    subclass,
                    prog_if,
                });
            }
        }
    }
    None
}
