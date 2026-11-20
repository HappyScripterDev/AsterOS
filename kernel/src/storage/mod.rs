//! Persistent storage: block devices, GPT partitions and the FAT32 volume.

pub mod fat32;
pub mod gpt;

use crate::drivers::ahci;

/// Initialise the storage stack and mount the persistent home volume.
///
/// Failure is not fatal: the system always boots with the in-memory root.
pub fn init() -> bool {
    if !ahci::init() {
        crate::warn!("storage: no usable disk, persistence disabled");
        return false;
    }

    let disks = ahci::disk_count();
    for disk in 0..disks {
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
                        if fat32::self_test() {
                            crate::info!("storage: FAT32 read self-test passed");
                        } else {
                            crate::error!("storage: FAT32 read self-test FAILED");
                        }
                        return true;
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
