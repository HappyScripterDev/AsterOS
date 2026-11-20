//! Mount table: filesystem instances and path-prefix routing.
//!
//! The VFS asks this table which filesystem owns a path. Longest-prefix wins,
//! so `/` (ramfs) covers system files while `/home/user` (FAT32) owns the
//! persistent home. Mount entries carry the device and partition identity so
//! Settings and Monitor can report real storage state.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::sync::IrqLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FsKind {
    Ramfs,
    Fat32,
}

#[derive(Clone)]
pub struct Mount {
    pub point: String,
    pub kind: FsKind,
    pub device: Option<usize>,
    pub partition_start: u64,
    pub partition_sectors: u64,
    pub label: String,
    pub read_only: bool,
}

#[derive(Clone, Copy)]
pub struct FatMount {
    pub device: usize,
    pub partition_start: u64,
    pub partition_sectors: u64,
    pub label: [u8; 12],
    pub read_only: bool,
}

static MOUNTS: IrqLock<Vec<Mount>> = IrqLock::new(Vec::new());

pub fn init_root() {
    let mut mounts = MOUNTS.lock();
    if mounts.iter().any(|mount| mount.point == "/") {
        return;
    }
    mounts.push(Mount {
        point: "/".to_string(),
        kind: FsKind::Ramfs,
        device: None,
        partition_start: 0,
        partition_sectors: 0,
        label: "ramfs".to_string(),
        read_only: false,
    });
}

pub fn attach(mount: Mount) -> Result<(), i64> {
    use aster_abi::errno;
    if mount.point.is_empty() || !mount.point.starts_with('/') {
        return Err(errno::EINVAL);
    }
    let mut mounts = MOUNTS.lock();
    if mounts.iter().any(|existing| existing.point == mount.point) {
        return Err(errno::EEXIST);
    }
    crate::info!(
        "mount: {} -> {:?} device {:?} label '{}'",
        mount.point,
        mount.kind,
        mount.device,
        mount.label
    );
    mounts.push(mount);
    Ok(())
}

/// Remove a mount point. Root cannot be detached. Callers must ensure no open
/// file refers to the mount; there is no reference-counted unmount yet.
pub fn detach(point: &str) -> Result<(), i64> {
    use aster_abi::errno;
    if point == "/" {
        return Err(errno::EPERM);
    }
    let mut mounts = MOUNTS.lock();
    let Some(index) = mounts.iter().position(|mount| mount.point == point) else {
        return Err(errno::ENOENT);
    };
    mounts.remove(index);
    Ok(())
}

/// Return the owning filesystem and the byte length of its mount prefix.
pub fn resolve(path: &str) -> Option<(FsKind, usize)> {
    if !path.starts_with('/') {
        return None;
    }
    let mounts = MOUNTS.lock();
    let mut best: Option<(FsKind, usize)> = None;
    for mount in mounts.iter() {
        if mount.point == "/" {
            if best.is_none() {
                best = Some((mount.kind, 0));
            }
            continue;
        }
        let matches = path == mount.point
            || (path.starts_with(&mount.point)
                && path.as_bytes().get(mount.point.len()) == Some(&b'/'));
        if matches
            && best
                .map(|(_, length)| mount.point.len() > length)
                .unwrap_or(true)
        {
            best = Some((mount.kind, mount.point.len()));
        }
    }
    best
}

pub fn fat_mount() -> Option<FatMount> {
    let mounts = MOUNTS.lock();
    mounts
        .iter()
        .find(|mount| mount.kind == FsKind::Fat32)
        .map(|mount| {
            let mut label = [0u8; 12];
            for (index, byte) in mount.label.as_bytes().iter().take(11).enumerate() {
                label[index] = *byte;
            }
            FatMount {
                device: mount.device.unwrap_or(0),
                partition_start: mount.partition_start,
                partition_sectors: mount.partition_sectors,
                label,
                read_only: mount.read_only,
            }
        })
}

pub fn mount_count() -> usize {
    MOUNTS.lock().len()
}

/// Boot self-test: root resolves, the FAT home resolves to Fat32 when present,
/// and the longest prefix wins.
pub fn self_test() -> bool {
    let Some((root_kind, root_prefix)) = resolve("/etc/motd") else {
        crate::error!("mount: root does not resolve");
        return false;
    };
    if root_kind != FsKind::Ramfs || root_prefix != 0 {
        crate::error!("mount: root resolved incorrectly");
        return false;
    }
    if fat_mount().is_some() {
        match resolve("/home/user") {
            Some((FsKind::Fat32, 10)) => {}
            _ => {
                crate::error!("mount: /home/user did not resolve to the FAT mount");
                return false;
            }
        }
        match resolve("/home/user/NOTES") {
            Some((FsKind::Fat32, 10)) => {}
            _ => {
                crate::error!("mount: nested home path did not resolve to FAT");
                return false;
            }
        }
    }
    true
}
