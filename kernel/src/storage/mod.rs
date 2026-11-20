//! Persistent storage: block devices, GPT partitions and the mounted home.

pub mod fat32;
pub mod gpt;

use alloc::string::ToString;

use aster_abi::errno;

use crate::block;
use crate::mount::{self, FsKind, Mount};

/// Snapshot of the storage stack for `STORAGE_DETAILS`.
pub struct Details {
    pub mounted: bool,
    pub healthy: bool,
    pub filesystem: u32,
    pub device: u32,
    pub read_only: bool,
    pub sector_size: u32,
    pub cluster_bytes: u32,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub device_sectors: u64,
    pub partition_start: u64,
    pub partition_sectors: u64,
    pub stats: block::IoStats,
    pub label: [u8; 12],
}

/// Initialise the storage stack and mount the persistent home volume.
///
/// Failure is not fatal: the system always boots with the in-memory root.
pub fn init() -> bool {
    mount::init_root();
    if !block::init() {
        crate::warn!("storage: no usable disk, persistence disabled");
        return false;
    }
    let devices = block::count();
    crate::info!("storage: {} block device(s), root is ramfs", devices);
    if !block::self_test() {
        crate::error!("storage: block self-test failed");
    } else {
        crate::info!("storage: block self-test passed (bounds + flush)");
    }

    for disk in 0..devices {
        match gpt::find_partition(disk) {
            Ok(Some(partition)) => {
                crate::info!(
                    "storage: disk {} partition at LBA {} ({} sectors)",
                    disk,
                    partition.start,
                    partition.sectors
                );
                match fat32::mount(disk, partition.start, partition.sectors) {
                    Ok(info) => {
                        crate::info!(
                            "storage: FAT32 '{}' mounted, {} clusters of {} bytes, {} MiB",
                            info.label,
                            info.clusters,
                            info.cluster_bytes,
                            info.clusters as u64 * info.cluster_bytes as u64 / (1024 * 1024)
                        );
                        if !fat32::self_test() {
                            crate::error!("storage: FAT32 self-test FAILED");
                        } else {
                            crate::info!("storage: FAT32 read self-test passed");
                        }
                        let mount_result = mount::attach(Mount {
                            point: "/home/user".to_string(),
                            kind: FsKind::Fat32,
                            device: Some(disk),
                            partition_start: partition.start,
                            partition_sectors: partition.sectors,
                            label: info.label,
                            read_only: false,
                        });
                        match mount_result {
                            Ok(()) => {
                                if mount::self_test() {
                                    crate::info!("storage: mount self-test passed");
                                }
                                return true;
                            }
                            Err(code) => {
                                crate::error!("storage: cannot attach /home/user: {}", code)
                            }
                        }
                    }
                    Err(error) => {
                        crate::warn!(
                            "storage: disk {} is not a usable FAT32 volume: {}",
                            disk,
                            error
                        )
                    }
                }
            }
            Ok(None) => {
                crate::warn!("storage: disk {} has no Aster data partition", disk);
            }
            Err(error) => {
                crate::warn!("storage: disk {} partition table invalid: {}", disk, error);
            }
        }
    }
    crate::warn!("storage: no persistent volume mounted");
    false
}

pub fn mounted() -> bool {
    mount::fat_mount().is_some()
}

/// Flush the persistent device. Called by the SYNC syscall and after settings
/// replacement.
pub fn sync() -> i64 {
    match mount::fat_mount() {
        Some(fat) => block::flush(fat.device).map(|_| 0).unwrap_or(errno::EIO),
        None => 0,
    }
}

pub fn details() -> Details {
    let fat = mount::fat_mount();
    let usage = fat32::usage();
    let stats = block::stats();
    match (fat, usage) {
        (Some(fat), Some(usage)) => Details {
            mounted: true,
            healthy: true,
            filesystem: 1,
            device: fat.device as u32,
            read_only: fat.read_only,
            sector_size: block::SECTOR_SIZE as u32,
            cluster_bytes: usage.cluster_bytes,
            total_bytes: usage.total_bytes,
            free_bytes: usage.free_bytes,
            device_sectors: block::info(fat.device)
                .map(|device| device.sectors)
                .unwrap_or(0),
            partition_start: fat.partition_start,
            partition_sectors: fat.partition_sectors,
            stats,
            label: fat.label,
        },
        (Some(fat), None) => Details {
            mounted: true,
            healthy: false,
            filesystem: 1,
            device: fat.device as u32,
            read_only: fat.read_only,
            sector_size: block::SECTOR_SIZE as u32,
            cluster_bytes: 0,
            total_bytes: 0,
            free_bytes: 0,
            device_sectors: block::info(fat.device)
                .map(|device| device.sectors)
                .unwrap_or(0),
            partition_start: fat.partition_start,
            partition_sectors: fat.partition_sectors,
            stats,
            label: fat.label,
        },
        _ => Details {
            mounted: false,
            healthy: false,
            filesystem: 0,
            device: 0,
            read_only: true,
            sector_size: block::SECTOR_SIZE as u32,
            cluster_bytes: 0,
            total_bytes: 0,
            free_bytes: 0,
            device_sectors: 0,
            partition_start: 0,
            partition_sectors: 0,
            stats,
            label: [0; 12],
        },
    }
}
