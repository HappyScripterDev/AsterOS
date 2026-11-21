//! Intel 82540EM (e1000) network driver.
//!
//! The reference machine exposes an e1000 NIC through QEMU user-mode
//! networking. The driver uses MMIO, 32 RX descriptors and 8 TX descriptors in
//! physical frames, and polls completion. Interrupts are disabled because the
//! PIC has no routed PCI IRQ in this milestone; the network thread polls at a
//! bounded rate instead.

use alloc::vec::Vec;
use core::sync::atomic::{fence, AtomicBool, Ordering};

use crate::drivers::pci;
use crate::mm::{pmm, vmm};
use crate::sync::IrqLock;

const VENDOR: u16 = 0x8086;
const DEVICE_ID: u16 = 0x100E;

const REG_CTRL: u64 = 0x0000;
const REG_STATUS: u64 = 0x0008;
const REG_EERD: u64 = 0x0014;
const REG_IMC: u64 = 0x00D8;
const REG_RCTL: u64 = 0x0100;
const REG_TCTL: u64 = 0x0400;
const REG_RDBAL: u64 = 0x2800;
const REG_RDBAH: u64 = 0x2804;
const REG_RDLEN: u64 = 0x2808;
const REG_RDH: u64 = 0x2810;
const REG_RDT: u64 = 0x2818;
const REG_TDBAL: u64 = 0x3800;
const REG_TDBAH: u64 = 0x3804;
const REG_TDLEN: u64 = 0x3808;
const REG_TDH: u64 = 0x3810;
const REG_TDT: u64 = 0x3818;

const CTRL_RST: u32 = 1 << 26;
const CTRL_SLU: u32 = 1 << 6;
const STATUS_LU: u32 = 1 << 1;

const RCTL_EN: u32 = 1 << 1;
const RCTL_UPE: u32 = 1 << 3;
const RCTL_MPE: u32 = 1 << 4;
const RCTL_BAM: u32 = 1 << 15;
const RCTL_SECRC: u32 = 1 << 26;

const TCTL_EN: u32 = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;
const TCTL_CT: u32 = 0x10 << 4;
const TCTL_COLD: u32 = 0x40 << 12;

const RX_DESCRIPTORS: usize = 256;
const TX_DESCRIPTORS: usize = 32;
const RX_BUFFER_SIZE: usize = 2048;
const MAX_FRAME: usize = 1514;

struct E1000 {
    bar: u64,
    mac: [u8; 6],
    rx_desc: u64,
    rx_buffers: [u64; RX_DESCRIPTORS],
    rx_tail: usize,
    tx_desc: u64,
    tx_buffers: [u64; TX_DESCRIPTORS],
    tx_tail: usize,
    link_up: bool,
}

unsafe impl Send for E1000 {}

static NIC: IrqLock<Option<E1000>> = IrqLock::new(None);
static FIRST_TX: AtomicBool = AtomicBool::new(false);
static FIRST_RX: AtomicBool = AtomicBool::new(false);

impl E1000 {
    fn read(&self, offset: u64) -> u32 {
        unsafe { ((self.bar + offset) as *const u32).read_volatile() }
    }

    fn write(&self, offset: u64, value: u32) {
        unsafe { ((self.bar + offset) as *mut u32).write_volatile(value) }
    }

    fn eeprom_read(&self, address: u8) -> u16 {
        self.write(REG_EERD, 1 | ((address as u32) << 8));
        for _ in 0..100_000 {
            let value = self.read(REG_EERD);
            if value & (1 << 4) != 0 {
                return (value >> 16) as u16;
            }
            core::hint::spin_loop();
        }
        0
    }

    fn rx_descriptor(&self, index: usize) -> u64 {
        vmm::hhdm(self.rx_desc) + (index * 16) as u64
    }

    fn tx_descriptor(&self, index: usize) -> u64 {
        vmm::hhdm(self.tx_desc) + (index * 16) as u64
    }
}

unsafe fn write_u16(address: u64, value: u16) {
    (address as *mut u16).write_unaligned(value);
}

unsafe fn read_u16(address: u64) -> u16 {
    (address as *const u16).read_unaligned()
}

unsafe fn write_u64(address: u64, value: u64) {
    (address as *mut u64).write_unaligned(value);
}

fn allocate_frame() -> Option<u64> {
    pmm::alloc_zeroed_frame()
}

pub fn init() -> bool {
    let Some(device) = pci::find_device(VENDOR, DEVICE_ID) else {
        crate::info!("e1000: no 82540EM network adapter present");
        return false;
    };
    let Some(bar) = pci::memory_bar(&device, 0) else {
        crate::error!("e1000: BAR0 is not a memory BAR");
        return false;
    };
    if bar == 0 {
        crate::error!("e1000: BAR0 is unassigned");
        return false;
    }
    pci::enable_device(&device);
    let bar = match vmm::map_kernel_mmio(bar, 0x20000) {
        Ok(address) => address,
        Err(error) => {
            crate::error!("e1000: cannot map BAR0: {}", error);
            return false;
        }
    };

    let Some(rx_desc) = allocate_frame() else {
        return false;
    };
    let Some(tx_desc) = allocate_frame() else {
        pmm::free_frame(rx_desc);
        return false;
    };
    let mut rx_buffers = [0u64; RX_DESCRIPTORS];
    for buffer in &mut rx_buffers {
        let Some(frame) = allocate_frame() else {
            return false;
        };
        *buffer = frame;
    }
    let mut tx_buffers = [0u64; TX_DESCRIPTORS];
    for buffer in &mut tx_buffers {
        let Some(frame) = allocate_frame() else {
            return false;
        };
        *buffer = frame;
    }

    let mut nic = E1000 {
        bar,
        mac: [0; 6],
        rx_desc,
        rx_buffers,
        rx_tail: 0,
        tx_desc,
        tx_buffers,
        tx_tail: 0,
        link_up: false,
    };

    // Reset the controller, then enable the link.
    let control = nic.read(REG_CTRL);
    nic.write(REG_CTRL, control | CTRL_RST);
    for _ in 0..1_000_000 {
        if nic.read(REG_CTRL) & CTRL_RST == 0 {
            break;
        }
        core::hint::spin_loop();
    }
    nic.write(REG_CTRL, nic.read(REG_CTRL) | CTRL_SLU);
    nic.write(REG_IMC, 0xFFFF_FFFF);

    let mac0 = nic.eeprom_read(0);
    let mac1 = nic.eeprom_read(1);
    let mac2 = nic.eeprom_read(2);
    nic.mac = [
        mac0 as u8,
        (mac0 >> 8) as u8,
        mac1 as u8,
        (mac1 >> 8) as u8,
        mac2 as u8,
        (mac2 >> 8) as u8,
    ];

    // RX descriptor ring and buffers.
    for index in 0..RX_DESCRIPTORS {
        let descriptor = nic.rx_descriptor(index);
        unsafe {
            write_u64(descriptor, rx_buffers[index]);
            write_u16(descriptor + 8, 0);
            write_u16(descriptor + 10, 0);
            *(descriptor as *mut u8).add(12) = 0;
            *(descriptor as *mut u8).add(13) = 0;
            write_u16(descriptor + 14, 0);
        }
    }
    nic.write(REG_RDBAL, (rx_desc & 0xFFFF_FFFF) as u32);
    nic.write(REG_RDBAH, (rx_desc >> 32) as u32);
    nic.write(REG_RDLEN, (RX_DESCRIPTORS * 16) as u32);
    nic.write(REG_RDH, 0);
    nic.write(REG_RDT, (RX_DESCRIPTORS - 1) as u32);
    nic.write(
        REG_RCTL,
        RCTL_EN | RCTL_UPE | RCTL_MPE | RCTL_BAM | RCTL_SECRC,
    );

    // TX descriptor ring and buffers. Legacy layout: length at 8, cso at 10,
    // cmd at 11, status at 12, css at 13, special at 14.
    for index in 0..TX_DESCRIPTORS {
        let descriptor = nic.tx_descriptor(index);
        unsafe {
            write_u64(descriptor, tx_buffers[index]);
            write_u16(descriptor + 8, 0);
            *(descriptor as *mut u8).add(11) = 0;
            *(descriptor as *mut u8).add(12) = 0;
            *(descriptor as *mut u8).add(13) = 0;
            write_u16(descriptor + 14, 0);
        }
    }
    nic.write(REG_TDBAL, (tx_desc & 0xFFFF_FFFF) as u32);
    nic.write(REG_TDBAH, (tx_desc >> 32) as u32);
    nic.write(REG_TDLEN, (TX_DESCRIPTORS * 16) as u32);
    nic.write(REG_TDH, 0);
    nic.write(REG_TDT, 0);
    nic.write(REG_TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);

    nic.link_up = nic.read(REG_STATUS) & STATUS_LU != 0;
    crate::info!(
        "e1000: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}, link {} (ctrl {:#x}, status {:#x})",
        nic.mac[0],
        nic.mac[1],
        nic.mac[2],
        nic.mac[3],
        nic.mac[4],
        nic.mac[5],
        if nic.link_up { "up" } else { "down" },
        nic.read(REG_CTRL),
        nic.read(REG_STATUS)
    );

    *NIC.lock() = Some(nic);
    true
}

pub fn present() -> bool {
    NIC.lock().is_some()
}

pub fn mac() -> Option<[u8; 6]> {
    NIC.lock().as_ref().map(|device| device.mac)
}

pub fn link_up() -> bool {
    let mut guard = NIC.lock();
    if let Some(device) = guard.as_mut() {
        device.link_up = device.read(REG_STATUS) & STATUS_LU != 0;
        device.link_up
    } else {
        false
    }
}

pub fn send(frame: &[u8]) -> Result<(), i64> {
    if frame.len() < 14 || frame.len() > MAX_FRAME {
        return Err(aster_abi::errno::EINVAL);
    }
    let mut guard = NIC.lock();
    let device = guard.as_mut().ok_or(aster_abi::errno::EIO)?;
    let index = device.tx_tail;
    let descriptor = device.tx_descriptor(index);
    let buffer = device.tx_buffers[index];

    // Transmits are synchronous: the previous call waited for the descriptor's
    // DD bit and cleared it, so this descriptor is free.
    let destination =
        unsafe { core::slice::from_raw_parts_mut(vmm::hhdm(buffer) as *mut u8, RX_BUFFER_SIZE) };
    destination[..frame.len()].copy_from_slice(frame);
    unsafe {
        write_u64(descriptor, buffer);
        write_u16(descriptor + 8, frame.len() as u16);
        write_u16(descriptor + 10, 0);
        // Transmit descriptor: cmd at 11, status at 12.
        *(descriptor as *mut u8).add(11) = 0x0B; // EOP | IFCS | RS
        *(descriptor as *mut u8).add(12) = 0;
        *(descriptor as *mut u8).add(13) = 0;
    }
    fence(Ordering::SeqCst);
    device.tx_tail = (index + 1) % TX_DESCRIPTORS;
    device.write(REG_TDT, device.tx_tail as u32);
    fence(Ordering::SeqCst);

    for _ in 0..1_000_000 {
        let status = unsafe { *(descriptor as *const u8).add(12) };
        if status & 1 != 0 {
            unsafe {
                *(descriptor as *mut u8).add(12) = 0;
            }
            if !FIRST_TX.swap(true, Ordering::Relaxed) {
                crate::info!("e1000: first TX completed ({} bytes)", frame.len());
            }
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(aster_abi::errno::EIO)
}

pub fn receive() -> Option<Vec<u8>> {
    let mut guard = NIC.lock();
    let device = guard.as_mut()?;
    let index = device.rx_tail;
    let descriptor = device.rx_descriptor(index);
    let status = unsafe { *(descriptor as *const u8).add(12) };
    if status & 1 == 0 {
        return None;
    }
    let length = unsafe { read_u16(descriptor + 8) } as usize;
    if !(14..=RX_BUFFER_SIZE).contains(&length) {
        unsafe {
            *(descriptor as *mut u8).add(12) = 0;
        }
        device.rx_tail = (index + 1) % RX_DESCRIPTORS;
        device.write(REG_RDT, index as u32);
        return None;
    }
    let source = unsafe {
        core::slice::from_raw_parts(vmm::hhdm(device.rx_buffers[index]) as *const u8, length)
    };
    let packet = source.to_vec();
    if !FIRST_RX.swap(true, Ordering::Relaxed) {
        crate::info!("e1000: first RX ({} bytes)", length);
    }
    unsafe {
        *(descriptor as *mut u8).add(12) = 0;
    }
    fence(Ordering::SeqCst);
    device.rx_tail = (index + 1) % RX_DESCRIPTORS;
    device.write(REG_RDT, index as u32);
    Some(packet)
}
