//! Block-device registry and bounds-checked sector I/O.
//!
//! The AHCI driver owns the controller; this layer owns device identity,
//! capacity checks, error normalisation and I/O accounting. Filesystems talk
//! only to this API, so a future NVMe/VirtIO backend can be added without
//! changing FAT or GPT.

use alloc::vec::Vec;

use crate::drivers::ahci;
use crate::sync::IrqLock;

pub const SECTOR_SIZE: usize = 512;

#[derive(Clone, Copy)]
pub struct DeviceInfo {
    pub port: u8,
    pub sectors: u64,
    pub model: [u8; 40],
}

#[derive(Clone, Copy, Default)]
pub struct IoStats {
    pub reads: u64,
    pub writes: u64,
    pub sectors_read: u64,
    pub sectors_written: u64,
    pub flushes: u64,
    pub errors: u64,
}

struct Registry {
    devices: Vec<DeviceInfo>,
    stats: IoStats,
}

static REGISTRY: IrqLock<Registry> = IrqLock::new(Registry {
    devices: Vec::new(),
    stats: IoStats {
        reads: 0,
        writes: 0,
        sectors_read: 0,
        sectors_written: 0,
        flushes: 0,
        errors: 0,
    },
});

/// Initialise AHCI and publish every attached ATA disk.
pub fn init() -> bool {
    if !ahci::init() {
        return false;
    }
    let count = ahci::disk_count();
    let mut devices = Vec::new();
    for index in 0..count {
        if let Some((port, sectors, model)) = ahci::disk_info(index) {
            let name = core::str::from_utf8(&model).unwrap_or("?").trim();
            crate::info!(
                "block: disk {} port {} '{}', {} sectors ({} MiB), {} bytes/sector",
                index,
                port,
                name,
                sectors,
                sectors * SECTOR_SIZE as u64 / (1024 * 1024),
                SECTOR_SIZE
            );
            devices.push(DeviceInfo {
                port,
                sectors,
                model,
            });
        }
    }
    let ready = !devices.is_empty();
    REGISTRY.lock().devices = devices;
    ready
}

pub fn count() -> usize {
    REGISTRY.lock().devices.len()
}

pub fn info(index: usize) -> Option<DeviceInfo> {
    REGISTRY.lock().devices.get(index).copied()
}

pub fn capacity_bytes(index: usize) -> u64 {
    info(index)
        .map(|device| device.sectors * SECTOR_SIZE as u64)
        .unwrap_or(0)
}

fn account(error: bool, update: impl FnOnce(&mut IoStats)) {
    let mut registry = REGISTRY.lock();
    if error {
        registry.stats.errors += 1;
    }
    update(&mut registry.stats);
}

pub fn read(index: usize, lba: u64, out: &mut [u8; SECTOR_SIZE]) -> Result<(), i64> {
    let device = info(index).ok_or(aster_abi::errno::EINVAL)?;
    if lba >= device.sectors {
        account(true, |_| {});
        return Err(aster_abi::errno::EINVAL);
    }
    match ahci::read_sector(index, lba, out) {
        Ok(()) => {
            account(false, |stats| {
                stats.reads += 1;
                stats.sectors_read += 1;
            });
            Ok(())
        }
        Err(_) => {
            account(true, |_| {});
            Err(aster_abi::errno::EIO)
        }
    }
}

pub fn write(index: usize, lba: u64, data: &[u8; SECTOR_SIZE]) -> Result<(), i64> {
    let device = info(index).ok_or(aster_abi::errno::EINVAL)?;
    if lba >= device.sectors {
        account(true, |_| {});
        return Err(aster_abi::errno::EINVAL);
    }
    match ahci::write_sector(index, lba, data) {
        Ok(()) => {
            account(false, |stats| {
                stats.writes += 1;
                stats.sectors_written += 1;
            });
            Ok(())
        }
        Err(_) => {
            account(true, |_| {});
            Err(aster_abi::errno::EIO)
        }
    }
}

pub fn flush(index: usize) -> Result<(), i64> {
    info(index).ok_or(aster_abi::errno::EINVAL)?;
    match ahci::flush_disk(index) {
        Ok(()) => {
            account(false, |stats| stats.flushes += 1);
            Ok(())
        }
        Err(_) => {
            account(true, |_| {});
            Err(aster_abi::errno::EIO)
        }
    }
}

pub fn stats() -> IoStats {
    REGISTRY.lock().stats
}

/// Boot self-test: capacity and bounds are enforced; flush is available.
/// It never writes the boot disk.
pub fn self_test() -> bool {
    if count() == 0 {
        crate::warn!("block: self-test skipped (no devices)");
        return true;
    }
    let mut sector = [0u8; SECTOR_SIZE];
    if read(0, 0, &mut sector).is_err() {
        crate::error!("block: cannot read LBA 0");
        return false;
    }
    let sectors = info(0).map(|device| device.sectors).unwrap_or(0);
    if sectors == 0 || read(0, sectors, &mut sector).is_ok() {
        crate::error!("block: out-of-range read was not rejected");
        return false;
    }
    if flush(0).is_err() {
        crate::error!("block: flush failed");
        return false;
    }
    true
}
