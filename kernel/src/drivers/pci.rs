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
