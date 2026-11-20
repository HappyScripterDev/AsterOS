//! AHCI (SATA) block driver, polling mode, one command slot.
//!
//! The q35 machine exposes an ICH9 AHCI controller. Disks attached with
//! `-drive if=ide,index=N` appear on port N. This driver performs IDENTIFY and
//! single-sector reads/writes; multi-sector DMA is deliberately avoided until
//! a physically-contiguous buffer allocator exists.

use alloc::vec::Vec;
use core::sync::atomic::{fence, Ordering};

use crate::drivers::pci;
use crate::mm::{pmm, vmm};
use crate::sync::IrqLock;

const HBA_CAP: u64 = 0x00;
const HBA_GHC: u64 = 0x04;
const HBA_PI: u64 = 0x0C;
const HBA_AE: u32 = 1 << 31;

const PORT_CLB: u64 = 0x00;
const PORT_CLBU: u64 = 0x04;
const PORT_FB: u64 = 0x08;
const PORT_FBU: u64 = 0x0C;
const PORT_IS: u64 = 0x10;
const PORT_CMD: u64 = 0x18;
const PORT_TFD: u64 = 0x20;
const PORT_SIG: u64 = 0x24;
const PORT_SSTS: u64 = 0x28;
const PORT_SERR: u64 = 0x30;
const PORT_CI: u64 = 0x38;

const CMD_ST: u32 = 1 << 0;
const CMD_FRE: u32 = 1 << 4;
const CMD_CR: u32 = 1 << 15;
const CMD_FR: u32 = 1 << 14;

const IS_TFES: u32 = 1 << 17;
const TFD_ERR: u32 = 1 << 0;

const FIS_TYPE_H2D: u8 = 0x27;
const ATA_IDENTIFY: u8 = 0xEC;
const ATA_READ_DMA_EXT: u8 = 0x25;
const ATA_WRITE_DMA_EXT: u8 = 0x35;
const ATA_FLUSH_CACHE_EXT: u8 = 0xEA;

const WAIT_SPINS: usize = 2_000_000;

struct Disk {
    port: u8,
    regs: u64,
    clb: u64,
    fis: u64,
    ct: u64,
    buf: u64,
    sectors: u64,
    model: [u8; 40],
}

impl Disk {
    fn buf_va(&self) -> u64 {
        vmm::hhdm(self.buf)
    }

    fn reg(&self, offset: u64) -> u32 {
        unsafe { ((self.regs + offset) as *const u32).read_volatile() }
    }

    fn set_reg(&self, offset: u64, value: u32) {
        unsafe { ((self.regs + offset) as *mut u32).write_volatile(value) }
    }

    /// Issue one ATA command whose data stage is a single 512-byte sector.
    fn issue(&self, command: u8, lba: u64, count: u16, write: bool) -> Result<(), &'static str> {
        // Command table: 0x00 CFIS, 0x40 ACMD, 0x80 PRDT.
        let table = unsafe { core::slice::from_raw_parts_mut(vmm::hhdm(self.ct) as *mut u8, 256) };
        for byte in table.iter_mut() {
            *byte = 0;
        }
        table[0] = FIS_TYPE_H2D;
        table[1] = 0x80; // command
        table[2] = command;
        table[4] = lba as u8;
        table[5] = (lba >> 8) as u8;
        table[6] = (lba >> 16) as u8;
        table[7] = 0x40; // LBA mode
        table[8] = (lba >> 24) as u8;
        table[9] = (lba >> 32) as u8;
        table[10] = (lba >> 40) as u8;
        table[12] = count as u8;
        table[13] = (count >> 8) as u8;

        // One PRDT entry pointing at the DMA buffer.
        let prdt = 0x80usize;
        table[prdt..prdt + 8].copy_from_slice(&self.buf.to_le_bytes());
        table[prdt + 8..prdt + 12].copy_from_slice(&0u32.to_le_bytes());
        let info: u32 = 512 | (1 << 31); // byte count | interrupt on completion
        table[prdt + 12..prdt + 16].copy_from_slice(&info.to_le_bytes());

        // Command header, slot 0.
        let header = unsafe { core::slice::from_raw_parts_mut(vmm::hhdm(self.clb) as *mut u32, 8) };
        for word in header.iter_mut() {
            *word = 0;
        }
        header[0] = 5 | if write { 1 << 6 } else { 0 } | (1 << 16);
        header[2] = self.ct as u32;
        header[3] = (self.ct >> 32) as u32;

        fence(Ordering::SeqCst);
        self.set_reg(PORT_IS, 0xFFFF_FFFF);
        self.set_reg(PORT_CI, 1);
        fence(Ordering::SeqCst);

        self.wait_completion(command)
    }

    fn wait_completion(&self, command: u8) -> Result<(), &'static str> {
        let mut completed = false;
        for _ in 0..WAIT_SPINS {
            if self.reg(PORT_CI) & 1 == 0 {
                completed = true;
                break;
            }
            let status = self.reg(PORT_IS);
            if status & IS_TFES != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        if !completed {
            crate::error!(
                "ahci: port {} command {:#x} timed out (is {:#x}, tfd {:#x})",
                self.port,
                command,
                self.reg(PORT_IS),
                self.reg(PORT_TFD)
            );
            return Err("AHCI command timed out");
        }
        fence(Ordering::SeqCst);
        let status = self.reg(PORT_IS);
        let task_file = self.reg(PORT_TFD);
        if status & IS_TFES != 0 || task_file & TFD_ERR != 0 {
            crate::error!(
                "ahci: port {} command {:#x} failed (is {:#x}, tfd {:#x}, serr {:#x})",
                self.port,
                command,
                status,
                task_file,
                self.reg(PORT_SERR)
            );
            return Err("AHCI command failed");
        }
        Ok(())
    }

    /// FLUSH CACHE EXT: no data stage and no PRDT entry.
    fn flush(&self) -> Result<(), &'static str> {
        let table = unsafe { core::slice::from_raw_parts_mut(vmm::hhdm(self.ct) as *mut u8, 256) };
        for byte in table.iter_mut() {
            *byte = 0;
        }
        table[0] = FIS_TYPE_H2D;
        table[1] = 0x80;
        table[2] = ATA_FLUSH_CACHE_EXT;
        table[7] = 0x40;

        let header = unsafe { core::slice::from_raw_parts_mut(vmm::hhdm(self.clb) as *mut u32, 8) };
        for word in header.iter_mut() {
            *word = 0;
        }
        header[0] = 5;
        header[2] = self.ct as u32;
        header[3] = (self.ct >> 32) as u32;

        fence(Ordering::SeqCst);
        self.set_reg(PORT_IS, 0xFFFF_FFFF);
        self.set_reg(PORT_CI, 1);
        fence(Ordering::SeqCst);
        self.wait_completion(ATA_FLUSH_CACHE_EXT)
    }

    fn read_sector(&self, lba: u64, out: &mut [u8; 512]) -> Result<(), &'static str> {
        if lba >= self.sectors {
            return Err("sector out of range");
        }
        self.issue(ATA_READ_DMA_EXT, lba, 1, false)?;
        let source = unsafe { core::slice::from_raw_parts(vmm::hhdm(self.buf) as *const u8, 512) };
        out.copy_from_slice(source);
        Ok(())
    }

    fn write_sector(&self, lba: u64, data: &[u8; 512]) -> Result<(), &'static str> {
        if lba >= self.sectors {
            return Err("sector out of range");
        }
        let destination =
            unsafe { core::slice::from_raw_parts_mut(vmm::hhdm(self.buf) as *mut u8, 512) };
        destination.copy_from_slice(data);
        fence(Ordering::SeqCst);
        self.issue(ATA_WRITE_DMA_EXT, lba, 1, true)
    }
}

struct Controller {
    disks: Vec<Disk>,
}

// The controller is only touched through `IrqLock`, which serializes access.
unsafe impl Send for Controller {}

static CONTROLLER: IrqLock<Option<Controller>> = IrqLock::new(None);

fn start_port(regs: u64, clb: u64, fis: u64) -> Result<(), &'static str> {
    let read = |offset: u64| unsafe { ((regs + offset) as *const u32).read_volatile() };
    let write =
        |offset: u64, value: u32| unsafe { ((regs + offset) as *mut u32).write_volatile(value) };

    let mut command = read(PORT_CMD);
    command &= !CMD_ST;
    write(PORT_CMD, command);
    for _ in 0..WAIT_SPINS {
        if read(PORT_CMD) & CMD_CR == 0 {
            break;
        }
        core::hint::spin_loop();
    }
    command &= !CMD_FRE;
    write(PORT_CMD, command);
    for _ in 0..WAIT_SPINS {
        if read(PORT_CMD) & CMD_FR == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    write(PORT_CLB, (clb & 0xFFFF_FFFF) as u32);
    write(PORT_CLBU, (clb >> 32) as u32);
    write(PORT_FB, (fis & 0xFFFF_FFFF) as u32);
    write(PORT_FBU, (fis >> 32) as u32);
    write(PORT_IS, 0xFFFF_FFFF);
    write(PORT_SERR, 0xFFFF_FFFF);

    let command = read(PORT_CMD) | CMD_FRE;
    write(PORT_CMD, command);
    write(PORT_CMD, command | CMD_ST);
    Ok(())
}

fn identify(regs: u64, clb: u64, fis: u64, ct: u64, buf: u64, port: u8) -> Option<(u64, [u8; 40])> {
    let disk = Disk {
        port,
        regs,
        clb,
        fis,
        ct,
        buf,
        sectors: 0,
        model: [0; 40],
    };
    if disk.issue(ATA_IDENTIFY, 0, 0, false).is_err() {
        return None;
    }
    let words = unsafe { core::slice::from_raw_parts(vmm::hhdm(buf) as *const u16, 256) };
    let lba28 = (words[60] as u64) | ((words[61] as u64) << 16);
    let lba48 = (words[100] as u64)
        | ((words[101] as u64) << 16)
        | ((words[102] as u64) << 32)
        | ((words[103] as u64) << 48);
    let supports_lba48 = words[83] & (1 << 10) != 0;
    let sectors = if supports_lba48 && lba48 > 0 {
        lba48
    } else {
        lba28
    };
    let mut model = [0u8; 40];
    for index in 0..20 {
        let word = words[27 + index].to_be_bytes();
        model[index * 2] = word[0];
        model[index * 2 + 1] = word[1];
    }
    Some((sectors, model))
}

/// Discover the AHCI controller and every attached ATA disk.
pub fn init() -> bool {
    let Some(device) = pci::find_ahci() else {
        crate::warn!("ahci: no SATA controller on bus 0");
        return false;
    };
    let Some(bar) = pci::memory_bar(&device, 5) else {
        crate::error!("ahci: controller has no memory BAR");
        return false;
    };
    if bar == 0 {
        crate::error!("ahci: controller BAR is unassigned");
        return false;
    }
    pci::enable_device(&device);
    let abar = match vmm::map_kernel_mmio(bar, 0x1100) {
        Ok(address) => address,
        Err(error) => {
            crate::error!("ahci: cannot map BAR {:#x}: {}", bar, error);
            return false;
        }
    };

    unsafe {
        let ghc = ((abar + HBA_GHC) as *const u32).read_volatile();
        ((abar + HBA_GHC) as *mut u32).write_volatile(ghc | HBA_AE);
    }
    let cap = unsafe { ((abar + HBA_CAP) as *const u32).read_volatile() };
    let ports = unsafe { ((abar + HBA_PI) as *const u32).read_volatile() };
    let slots = ((cap >> 8) & 0x1F) + 1;
    crate::info!(
        "ahci: {}:{}.{} vendor {:04x} device {:04x}, {} ports, {} slots",
        device.bus,
        device.device,
        device.function,
        device.vendor,
        device.id,
        (cap & 0x1F) + 1,
        slots
    );

    let mut disks = Vec::new();
    for port in 0..32u8 {
        if ports & (1 << port) == 0 {
            continue;
        }
        let regs = abar + 0x100 + port as u64 * 0x80;
        let ssts = unsafe { ((regs + PORT_SSTS) as *const u32).read_volatile() };
        if ssts & 0x0F != 3 {
            continue;
        }
        let signature = unsafe { ((regs + PORT_SIG) as *const u32).read_volatile() };
        if signature != 0x101 {
            crate::warn!(
                "ahci: port {} signature {:#x} is not an ATA disk",
                port,
                signature
            );
            continue;
        }
        let Some(clb) = pmm::alloc_zeroed_frame() else {
            crate::error!("ahci: out of memory for a command list");
            break;
        };
        let Some(fis) = pmm::alloc_zeroed_frame() else {
            pmm::free_frame(clb);
            break;
        };
        let Some(ct) = pmm::alloc_zeroed_frame() else {
            pmm::free_frame(clb);
            pmm::free_frame(fis);
            break;
        };
        let Some(buf) = pmm::alloc_zeroed_frame() else {
            pmm::free_frame(clb);
            pmm::free_frame(fis);
            pmm::free_frame(ct);
            break;
        };

        if let Err(error) = start_port(regs, clb, fis) {
            crate::error!("ahci: port {} cannot start: {}", port, error);
            pmm::free_frame(clb);
            pmm::free_frame(fis);
            pmm::free_frame(ct);
            pmm::free_frame(buf);
            continue;
        }
        match identify(regs, clb, fis, ct, buf, port) {
            Some((sectors, model)) => {
                let name = core::str::from_utf8(&model).unwrap_or("?").trim();
                crate::info!(
                    "ahci: port {} disk '{}', {} sectors ({} MiB)",
                    port,
                    name,
                    sectors,
                    sectors * 512 / (1024 * 1024)
                );
                disks.push(Disk {
                    port,
                    regs,
                    clb,
                    fis,
                    ct,
                    buf,
                    sectors,
                    model,
                });
            }
            None => {
                crate::warn!("ahci: port {} IDENTIFY failed", port);
                pmm::free_frame(clb);
                pmm::free_frame(fis);
                pmm::free_frame(ct);
                pmm::free_frame(buf);
            }
        }
    }

    let count = disks.len();
    *CONTROLLER.lock() = Some(Controller { disks });
    crate::info!("ahci: {} disk(s) ready", count);
    count > 0
}

pub fn disk_count() -> usize {
    CONTROLLER
        .lock()
        .as_ref()
        .map(|controller| controller.disks.len())
        .unwrap_or(0)
}

pub fn disk_sectors(index: usize) -> u64 {
    CONTROLLER
        .lock()
        .as_ref()
        .and_then(|controller| controller.disks.get(index))
        .map(|disk| disk.sectors)
        .unwrap_or(0)
}

pub fn disk_info(index: usize) -> Option<(u8, u64, [u8; 40])> {
    CONTROLLER
        .lock()
        .as_ref()
        .and_then(|controller| controller.disks.get(index))
        .map(|disk| (disk.port, disk.sectors, disk.model))
}

pub fn flush_disk(index: usize) -> Result<(), &'static str> {
    let controller = CONTROLLER.lock();
    let disk = controller
        .as_ref()
        .and_then(|controller| controller.disks.get(index))
        .ok_or("no such disk")?;
    disk.flush()
}

pub fn read_sector(index: usize, lba: u64, out: &mut [u8; 512]) -> Result<(), &'static str> {
    let controller = CONTROLLER.lock();
    let disk = controller
        .as_ref()
        .and_then(|controller| controller.disks.get(index))
        .ok_or("no such disk")?;
    disk.read_sector(lba, out)
}

pub fn write_sector(index: usize, lba: u64, data: &[u8; 512]) -> Result<(), &'static str> {
    let controller = CONTROLLER.lock();
    let disk = controller
        .as_ref()
        .and_then(|controller| controller.disks.get(index))
        .ok_or("no such disk")?;
    disk.write_sector(lba, data)
}
