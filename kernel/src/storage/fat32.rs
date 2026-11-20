//! FAT32 volume: mount, directory lookup, reads (writes are added incrementally).
//!
//! 8.3 names only for now: the on-disk builder and the kernel driver agree on
//! uppercase short names. Long-name entries are skipped, not misinterpreted.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::block;
use crate::sync::IrqLock;

use aster_abi::errno;

pub const MOUNT_TAG: u64 = 1 << 56;
const ATTR_DIR: u8 = 0x10;
const ATTR_LFN: u8 = 0x0F;
const ATTR_VOLUME: u8 = 0x08;
const ATTR_LOWERCASE: u8 = 0x18;
const FAT_EOC: u32 = 0x0FFF_FFFF;
const MAX_FILE_SIZE: usize = 4 * 1024 * 1024;

pub struct MountInfo {
    pub label: String,
    pub clusters: u32,
    pub cluster_bytes: u32,
}

pub struct Usage {
    pub label: String,
    pub cluster_bytes: u32,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

struct Fat {
    disk: usize,
    part_start: u64,
    spc: u8,
    reserved: u16,
    fats: u8,
    fat_sectors: u32,
    root_cluster: u32,
    total_sectors: u32,
    clusters: u32,
    label: String,
    next_free: u32,
    free_cache: Option<u32>,
}

static FAT: IrqLock<Option<Fat>> = IrqLock::new(None);

pub fn mounted() -> bool {
    FAT.lock().is_some()
}

fn with_fat<T>(function: impl FnOnce(&mut Fat) -> Result<T, i64>) -> Result<T, i64> {
    let mut guard = FAT.lock();
    let fat = guard.as_mut().ok_or(errno::EIO)?;
    function(fat)
}

fn read_bytes(sector: &[u8], offset: usize, length: usize) -> u32 {
    let mut value = 0u32;
    for index in 0..length {
        value |= (sector[offset + index] as u32) << (index * 8);
    }
    value
}

pub fn mount(disk: usize, part_start: u64, sectors: u64) -> Result<MountInfo, &'static str> {
    let mut boot = [0u8; 512];
    block::read(disk, part_start, &mut boot).map_err(|_| "block read failed")?;
    if boot[510] != 0x55 || boot[511] != 0xAA {
        return Err("missing FAT boot signature");
    }
    let bytes_per_sector = u16::from_le_bytes([boot[11], boot[12]]);
    if bytes_per_sector != 512 {
        return Err("unsupported sector size");
    }
    let spc = boot[13];
    if spc == 0 || spc > 128 || !spc.is_power_of_two() {
        return Err("invalid sectors per cluster");
    }
    let reserved = u16::from_le_bytes([boot[14], boot[15]]);
    let fats = boot[16];
    if fats == 0 || fats > 4 {
        return Err("invalid FAT count");
    }
    let root_entries = u16::from_le_bytes([boot[17], boot[18]]);
    if root_entries != 0 {
        return Err("not a FAT32 volume");
    }
    let total16 = u16::from_le_bytes([boot[19], boot[20]]) as u32;
    let total32 = read_bytes(&boot, 32, 4);
    let fat16 = u16::from_le_bytes([boot[22], boot[23]]);
    if fat16 != 0 {
        return Err("not a FAT32 volume (16-bit FAT)");
    }
    let fat_sectors = read_bytes(&boot, 36, 4);
    if fat_sectors == 0 {
        return Err("zero-sized FAT");
    }
    let total = if total16 != 0 { total16 } else { total32 };
    let total = total.min(sectors.min(u32::MAX as u64) as u32);
    let root_cluster = read_bytes(&boot, 44, 4);
    let first_data = reserved as u64 + fats as u64 * fat_sectors as u64;
    if first_data >= total as u64 || total < 64 {
        return Err("FAT geometry outside the partition");
    }
    let clusters = ((total as u64 - first_data) / spc as u64) as u32;
    if clusters < 16 || root_cluster < 2 || root_cluster >= clusters + 2 {
        return Err("invalid cluster count or root cluster");
    }

    let mut label_bytes = [0u8; 11];
    if boot[66] == 0x29 {
        label_bytes.copy_from_slice(&boot[71..82]);
    }
    let label = String::from_utf8_lossy(&label_bytes).trim().to_string();
    let label = if label.is_empty() {
        "NO NAME".to_string()
    } else {
        label
    };

    let info = MountInfo {
        label: label.clone(),
        clusters,
        cluster_bytes: spc as u32 * 512,
    };
    *FAT.lock() = Some(Fat {
        disk,
        part_start,
        spc,
        reserved,
        fats,
        fat_sectors,
        root_cluster,
        total_sectors: total,
        clusters,
        label,
        next_free: 2,
        free_cache: None,
    });
    Ok(info)
}

impl Fat {
    fn read_sector(&self, lba: u64, out: &mut [u8; 512]) -> Result<(), i64> {
        if lba >= self.total_sectors as u64 {
            return Err(errno::EIO);
        }
        block::read(self.disk, self.part_start + lba, out)
    }

    fn write_sector(&self, lba: u64, data: &[u8; 512]) -> Result<(), i64> {
        if lba >= self.total_sectors as u64 {
            return Err(errno::EIO);
        }
        block::write(self.disk, self.part_start + lba, data)
    }

    fn cluster_bytes(&self) -> u32 {
        self.spc as u32 * 512
    }

    fn cluster_lba(&self, cluster: u32) -> u64 {
        self.reserved as u64
            + self.fats as u64 * self.fat_sectors as u64
            + (cluster as u64 - 2) * self.spc as u64
    }

    fn fat_sector(&self, cluster: u32) -> (u64, usize) {
        let offset = cluster as u64 * 4;
        (self.reserved as u64 + offset / 512, (offset % 512) as usize)
    }

    fn fat_get(&self, cluster: u32) -> Result<u32, i64> {
        if cluster < 2 || cluster >= self.clusters + 2 {
            return Err(errno::EIO);
        }
        let (lba, offset) = self.fat_sector(cluster);
        let mut sector = [0u8; 512];
        self.read_sector(lba, &mut sector)?;
        Ok(read_bytes(&sector, offset, 4) & 0x0FFF_FFFF)
    }

    fn fat_set(&mut self, cluster: u32, value: u32) -> Result<(), i64> {
        if cluster < 2 || cluster >= self.clusters + 2 {
            return Err(errno::EIO);
        }
        let (lba, offset) = self.fat_sector(cluster);
        let mut sector = [0u8; 512];
        self.read_sector(lba, &mut sector)?;
        let previous = read_bytes(&sector, offset, 4);
        let updated = (previous & 0xF000_0000) | (value & 0x0FFF_FFFF);
        sector[offset..offset + 4].copy_from_slice(&updated.to_le_bytes());
        for copy in 0..self.fats {
            let target = lba + copy as u64 * self.fat_sectors as u64;
            self.write_sector(target, &sector)?;
        }
        Ok(())
    }

    /// Next cluster in a chain, or None at the end.
    fn next_cluster(&self, cluster: u32) -> Result<Option<u32>, i64> {
        let next = self.fat_get(cluster)?;
        if next >= FAT_EOC - 7 {
            return Ok(None);
        }
        if next < 2 || next >= self.clusters + 2 {
            return Err(errno::EIO);
        }
        Ok(Some(next))
    }

    /// Count free clusters by scanning FAT copy 0 sector by sector. The result
    /// is cached until an allocation or free changes the FAT.
    fn count_free(&self) -> Result<u32, i64> {
        let entries = self.clusters + 2;
        let sectors = ((entries as u64 * 4 + 511) / 512) as u64;
        let mut free = 0u32;
        for index in 0..sectors {
            let mut sector = [0u8; 512];
            self.read_sector(self.reserved as u64 + index, &mut sector)?;
            let first = if index == 0 { 2 } else { 0 };
            for slot in first..128usize {
                let entry = index as usize * 128 + slot;
                if entry >= entries as usize {
                    break;
                }
                let value = read_bytes(&sector, slot * 4, 4) & 0x0FFF_FFFF;
                if value == 0 {
                    free += 1;
                }
            }
        }
        Ok(free)
    }

    fn entry_sector(&self, inode: u64) -> Result<(u64, usize), i64> {
        if inode == 0 {
            return Err(errno::EINVAL);
        }
        let offset = inode - 1;
        let lba = offset / 512;
        if lba >= self.total_sectors as u64 {
            return Err(errno::ENOENT);
        }
        Ok((lba, (offset % 512) as usize))
    }

    fn read_entry(&self, inode: u64) -> Result<[u8; 32], i64> {
        let (lba, offset) = self.entry_sector(inode)?;
        let mut sector = [0u8; 512];
        self.read_sector(lba, &mut sector)?;
        let mut entry = [0u8; 32];
        entry.copy_from_slice(&sector[offset..offset + 32]);
        if entry[0] == 0 || entry[0] == 0xE5 {
            return Err(errno::ENOENT);
        }
        Ok(entry)
    }

    fn location(&self, inode: u64) -> Result<Location, i64> {
        if inode == 0 {
            return Ok(Location {
                first_cluster: self.root_cluster,
                size: 0,
                directory: true,
            });
        }
        let entry = self.read_entry(inode)?;
        let high = u16::from_le_bytes([entry[20], entry[21]]) as u32;
        let low = u16::from_le_bytes([entry[26], entry[27]]) as u32;
        Ok(Location {
            first_cluster: (high << 16) | low,
            size: read_bytes(&entry, 28, 4),
            directory: entry[11] & ATTR_DIR != 0,
        })
    }

    fn walk_dir(
        &self,
        start: u32,
        mut visit: impl FnMut(u32, u64, &[u8; 32]) -> Result<bool, i64>,
    ) -> Result<(), i64> {
        let mut cluster = start;
        let mut guard = 0u32;
        'clusters: loop {
            if cluster < 2 || cluster >= self.clusters + 2 {
                return Err(errno::EIO);
            }
            let base = self.cluster_lba(cluster);
            for sector_index in 0..self.spc as u64 {
                let lba = base + sector_index;
                let mut sector = [0u8; 512];
                self.read_sector(lba, &mut sector)?;
                for slot in 0..16usize {
                    let offset = slot * 32;
                    let mut entry = [0u8; 32];
                    entry.copy_from_slice(&sector[offset..offset + 32]);
                    if entry[0] == 0 {
                        return Ok(());
                    }
                    let volume_offset = lba * 512 + offset as u64;
                    if visit(cluster, volume_offset, &entry)? {
                        return Ok(());
                    }
                }
            }
            guard += 1;
            if guard > self.clusters + 2 {
                return Err(errno::EIO);
            }
            match self.next_cluster(cluster)? {
                Some(next) => cluster = next,
                None => break 'clusters,
            }
        }
        Ok(())
    }

    fn lookup_component(
        &self,
        directory_cluster: u32,
        name: &str,
    ) -> Result<Option<(u64, bool)>, i64> {
        let mut found = None;
        self.walk_dir(directory_cluster, |_, offset, entry| {
            if let Some(entry_name) = entry_name(entry) {
                if entry_name.eq_ignore_ascii_case(name) {
                    found = Some((offset + 1, entry[11] & ATTR_DIR != 0));
                    return Ok(true);
                }
            }
            Ok(false)
        })?;
        Ok(found)
    }

    fn cluster_of(entry: &[u8; 32]) -> u32 {
        let high = u16::from_le_bytes([entry[20], entry[21]]) as u32;
        let low = u16::from_le_bytes([entry[26], entry[27]]) as u32;
        (high << 16) | low
    }

    fn set_cluster(entry: &mut [u8; 32], cluster: u32) {
        entry[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
        entry[26..28].copy_from_slice(&((cluster & 0xFFFF) as u16).to_le_bytes());
    }

    fn file_size(entry: &[u8; 32]) -> u32 {
        read_bytes(entry, 28, 4)
    }

    fn set_size(entry: &mut [u8; 32], size: u32) {
        entry[28..32].copy_from_slice(&size.to_le_bytes());
    }

    fn stamp(entry: &mut [u8; 32]) {
        let now = crate::arch::rtc::read();
        let year = (now.year as u32).saturating_sub(1980).min(127);
        let date = (year << 9) | ((now.month as u32) << 5) | now.day as u32;
        let time =
            ((now.hour as u32) << 11) | ((now.minute as u32) << 5) | ((now.second as u32) / 2);
        entry[22..24].copy_from_slice(&(time as u16).to_le_bytes());
        entry[24..26].copy_from_slice(&(date as u16).to_le_bytes());
        entry[18..20].copy_from_slice(&(date as u16).to_le_bytes());
    }

    fn zero_cluster(&self, cluster: u32) -> Result<(), i64> {
        let zero = [0u8; 512];
        let base = self.cluster_lba(cluster);
        for sector in 0..self.spc as u64 {
            self.write_sector(base + sector, &zero)?;
        }
        Ok(())
    }

    fn alloc_cluster(&mut self) -> Result<u32, i64> {
        let end = self.clusters + 2;
        let start = self.next_free.max(2);
        for cluster in start..end {
            if self.fat_get(cluster)? == 0 {
                self.fat_set(cluster, FAT_EOC)?;
                self.next_free = cluster + 1;
                self.free_cache = None;
                self.zero_cluster(cluster)?;
                return Ok(cluster);
            }
        }
        for cluster in 2..start {
            if self.fat_get(cluster)? == 0 {
                self.fat_set(cluster, FAT_EOC)?;
                self.next_free = cluster + 1;
                self.free_cache = None;
                self.zero_cluster(cluster)?;
                return Ok(cluster);
            }
        }
        Err(errno::ENOSPC)
    }

    fn free_chain(&mut self, first: u32) -> Result<(), i64> {
        let mut cluster = first;
        let mut guard = 0u32;
        while cluster >= 2 && guard <= self.clusters {
            let next = self.fat_get(cluster)?;
            self.fat_set(cluster, 0)?;
            if next >= FAT_EOC - 7 || next < 2 {
                break;
            }
            cluster = next;
            guard += 1;
        }
        if self.next_free > first {
            self.next_free = first;
        }
        self.free_cache = None;
        Ok(())
    }

    /// Find a free directory slot. Returns the slot and the last cluster of
    /// the directory, so the caller can extend it when it is full.
    fn find_slot(&self, start: u32) -> Result<(Option<(u64, usize)>, u32), i64> {
        let mut cluster = start;
        let mut last = start;
        let mut deleted = None;
        let mut guard = 0u32;
        loop {
            last = cluster;
            let base = self.cluster_lba(cluster);
            for sector_index in 0..self.spc as u64 {
                let lba = base + sector_index;
                let mut sector = [0u8; 512];
                self.read_sector(lba, &mut sector)?;
                for slot in 0..16usize {
                    let offset = slot * 32;
                    if sector[offset] == 0 {
                        return Ok((Some((lba, offset)), last));
                    }
                    if sector[offset] == 0xE5 && deleted.is_none() {
                        deleted = Some((lba, offset));
                    }
                }
            }
            guard += 1;
            if guard > self.clusters + 2 {
                return Err(errno::EIO);
            }
            match self.next_cluster(cluster)? {
                Some(next) => cluster = next,
                None => break,
            }
        }
        Ok((deleted, last))
    }

    fn write_entry(&self, inode: u64, entry: &[u8; 32]) -> Result<(), i64> {
        let (lba, offset) = self.entry_sector(inode)?;
        let mut sector = [0u8; 512];
        self.read_sector(lba, &mut sector)?;
        sector[offset..offset + 32].copy_from_slice(entry);
        self.write_sector(lba, &sector)
    }

    fn create_inode(&mut self, parent: u32, name: &str, directory: bool) -> Result<u64, i64> {
        let short = make_83(name)?;
        let (slot, last) = self.find_slot(parent)?;
        let (lba, offset) = match slot {
            Some(slot) => slot,
            None => {
                let new = self.alloc_cluster()?;
                self.fat_set(last, new)?;
                (self.cluster_lba(new), 0)
            }
        };

        let mut entry = [0u8; 32];
        entry[0..11].copy_from_slice(&short);
        entry[11] = if directory { ATTR_DIR } else { 0x20 };
        Self::stamp(&mut entry);
        let cluster = if directory {
            let cluster = self.alloc_cluster()?;
            Self::set_cluster(&mut entry, cluster);
            cluster
        } else {
            0
        };
        self.write_entry_at(lba, offset, &entry)?;

        if directory {
            self.write_entry_at(self.cluster_lba(cluster), 0, &dir_entry(b".", cluster, 0))?;
            self.write_entry_at(self.cluster_lba(cluster), 32, &dir_entry(b"..", parent, 0))?;
        }

        Ok(lba * 512 + offset as u64 + 1)
    }

    fn write_entry_at(&self, lba: u64, offset: usize, entry: &[u8; 32]) -> Result<(), i64> {
        let mut sector = [0u8; 512];
        self.read_sector(lba, &mut sector)?;
        sector[offset..offset + 32].copy_from_slice(entry);
        self.write_sector(lba, &sector)
    }

    /// Write a byte range. `source` is None for a zero-filled gap.
    fn write_range(
        &self,
        first: u32,
        start: u64,
        end: u64,
        source: Option<&[u8]>,
        source_offset: usize,
    ) -> Result<(), i64> {
        let cluster_bytes = self.cluster_bytes() as u64;
        let mut cluster = first;
        let mut skip = start / cluster_bytes;
        while skip > 0 {
            cluster = self.next_cluster(cluster)?.ok_or(errno::EIO)?;
            skip -= 1;
        }
        let mut within = (start % cluster_bytes) as usize;
        let mut position = start;
        let mut consumed = source_offset;
        while position < end {
            let sector_index = within / 512;
            let sector_offset = within % 512;
            let lba = self.cluster_lba(cluster) + sector_index as u64;
            let mut sector = [0u8; 512];
            self.read_sector(lba, &mut sector)?;
            let chunk = (512 - sector_offset).min((end - position) as usize);
            match source {
                Some(bytes) => {
                    let available = bytes.len().saturating_sub(consumed);
                    let count = chunk.min(available);
                    if count > 0 {
                        sector[sector_offset..sector_offset + count]
                            .copy_from_slice(&bytes[consumed..consumed + count]);
                        consumed += count;
                    }
                    if count < chunk {
                        sector[sector_offset + count..sector_offset + chunk].fill(0);
                    }
                }
                None => sector[sector_offset..sector_offset + chunk].fill(0),
            }
            self.write_sector(lba, &sector)?;
            position += chunk as u64;
            within += chunk;
            if within >= cluster_bytes as usize && position < end {
                cluster = self.next_cluster(cluster)?.ok_or(errno::EIO)?;
                within -= cluster_bytes as usize;
            }
        }
        Ok(())
    }

    fn write_inode(&mut self, inode: u64, offset: u64, data: &[u8]) -> Result<usize, i64> {
        if data.is_empty() {
            return Ok(0);
        }
        let mut entry = self.read_entry(inode)?;
        if entry[11] & ATTR_DIR != 0 {
            return Err(errno::EISDIR);
        }
        let end = offset.checked_add(data.len() as u64).ok_or(errno::EINVAL)?;
        if end > MAX_FILE_SIZE as u64 {
            return Err(errno::ENOSPC);
        }
        let old_size = Self::file_size(&entry) as u64;
        let mut first = Self::cluster_of(&entry);
        if first < 2 {
            first = self.alloc_cluster()?;
            Self::set_cluster(&mut entry, first);
        }
        let cluster_bytes = self.cluster_bytes() as u64;
        let needed = ((end + cluster_bytes - 1) / cluster_bytes) as usize;
        let mut cluster = first;
        let mut have = 1usize;
        while have < needed {
            match self.next_cluster(cluster)? {
                Some(next) => cluster = next,
                None => {
                    let next = self.alloc_cluster()?;
                    self.fat_set(cluster, next)?;
                    cluster = next;
                }
            }
            have += 1;
        }

        if offset > old_size {
            self.write_range(first, old_size, offset, None, 0)?;
        }
        self.write_range(first, offset, end, Some(data), 0)?;
        Self::set_size(&mut entry, end as u32);
        Self::set_cluster(&mut entry, first);
        self.write_entry(inode, &entry)?;
        Ok(data.len())
    }

    fn truncate_inode(&mut self, inode: u64) -> Result<(), i64> {
        let mut entry = self.read_entry(inode)?;
        if entry[11] & ATTR_DIR != 0 {
            return Err(errno::EISDIR);
        }
        let cluster = Self::cluster_of(&entry);
        if cluster >= 2 {
            self.free_chain(cluster)?;
        }
        Self::set_cluster(&mut entry, 0);
        Self::set_size(&mut entry, 0);
        self.write_entry(inode, &entry)
    }

    fn mark_deleted(&self, inode: u64) -> Result<(), i64> {
        let (lba, offset) = self.entry_sector(inode)?;
        let mut sector = [0u8; 512];
        self.read_sector(lba, &mut sector)?;
        sector[offset] = 0xE5;
        self.write_sector(lba, &sector)
    }

    fn unlink_inode(&mut self, inode: u64, directory: bool, depth: u32) -> Result<(), i64> {
        if depth > 16 {
            return Err(errno::EINVAL);
        }
        if directory {
            let location = self.location(inode)?;
            let mut children = Vec::new();
            self.walk_dir(location.first_cluster, |_, offset, entry| {
                if entry_name(entry).is_some() {
                    children.push((offset + 1, entry[11] & ATTR_DIR != 0));
                }
                Ok(false)
            })?;
            for (child, child_directory) in children {
                self.unlink_inode(child, child_directory, depth + 1)?;
            }
        }
        let entry = self.read_entry(inode)?;
        let cluster = Self::cluster_of(&entry);
        if cluster >= 2 {
            self.free_chain(cluster)?;
        }
        self.mark_deleted(inode)
    }

    fn open_path(&mut self, path: &str, flags: u32) -> Result<u64, i64> {
        match self.lookup(path) {
            Ok(inode) => {
                let location = self.location(inode)?;
                if flags & aster_abi::open_flags::DIRECTORY != 0 && !location.directory {
                    return Err(errno::ENOTDIR);
                }
                if flags & aster_abi::open_flags::TRUNC != 0 {
                    if location.directory {
                        return Err(errno::EISDIR);
                    }
                    self.truncate_inode(inode)?;
                }
                Ok(inode)
            }
            Err(errno::ENOENT) if flags & aster_abi::open_flags::CREATE != 0 => {
                let (parent, name) = split_parent(path)?;
                let parent_inode = self.lookup(parent)?;
                let parent_location = self.location(parent_inode)?;
                if !parent_location.directory {
                    return Err(errno::ENOTDIR);
                }
                if self
                    .lookup_component(parent_location.first_cluster, name)?
                    .is_some()
                {
                    return Err(errno::EEXIST);
                }
                let directory = flags & aster_abi::open_flags::DIRECTORY != 0;
                self.create_inode(parent_location.first_cluster, name, directory)
            }
            Err(code) => Err(code),
        }
    }

    fn rename_path(&mut self, old_path: &str, new_path: &str) -> Result<(), i64> {
        let old_inode = self.lookup(old_path)?;
        if old_inode == 0 {
            return Err(errno::EPERM);
        }
        let (old_parent, _) = split_parent(old_path)?;
        let (new_parent, new_name) = split_parent(new_path)?;
        let old_parent_inode = self.lookup(old_parent)?;
        let new_parent_inode = self.lookup(new_parent)?;
        if old_parent_inode != new_parent_inode {
            return Err(errno::EINVAL);
        }
        let parent = self.location(new_parent_inode)?;
        if !parent.directory {
            return Err(errno::ENOTDIR);
        }
        let short = make_83(new_name)?;
        if let Some((existing, _)) = self.lookup_component(parent.first_cluster, new_name)? {
            if existing != old_inode {
                return Err(errno::EEXIST);
            }
            return Ok(());
        }
        let mut entry = self.read_entry(old_inode)?;
        entry[0..11].copy_from_slice(&short);
        // The short entry is now uppercase; clear the case flags.
        entry[12] = 0;
        self.write_entry(old_inode, &entry)
    }

    fn mkdir_path(&mut self, path: &str) -> Result<u64, i64> {
        if self.lookup(path).is_ok() {
            return Err(errno::EEXIST);
        }
        let (parent, name) = split_parent(path)?;
        let parent_inode = self.lookup(parent)?;
        let location = self.location(parent_inode)?;
        if !location.directory {
            return Err(errno::ENOTDIR);
        }
        self.create_inode(location.first_cluster, name, true)
    }

    fn unlink_path(&mut self, path: &str) -> Result<(), i64> {
        let inode = self.lookup(path)?;
        if inode == 0 {
            return Err(errno::EPERM);
        }
        let location = self.location(inode)?;
        self.unlink_inode(inode, location.directory, 0)
    }

    fn lookup(&self, path: &str) -> Result<u64, i64> {
        let mut current = 0u64;
        let mut cluster = self.root_cluster;
        for component in path.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component == ".." {
                return Err(errno::EINVAL);
            }
            let Some((inode, is_dir)) = self.lookup_component(cluster, component)? else {
                return Err(errno::ENOENT);
            };
            if is_dir {
                cluster = self.location(inode)?.first_cluster;
            }
            current = inode;
        }
        Ok(current)
    }
}

struct Location {
    first_cluster: u32,
    size: u32,
    directory: bool,
}

fn entry_name(entry: &[u8; 32]) -> Option<String> {
    let attribute = entry[11];
    if entry[0] == 0 || entry[0] == 0xE5 || attribute & ATTR_LFN == ATTR_LFN {
        return None;
    }
    if attribute & ATTR_VOLUME != 0 {
        return None;
    }
    let mut base = String::new();
    for index in 0..8 {
        let byte = entry[index];
        if byte == b' ' {
            break;
        }
        base.push(byte as char);
    }
    if base == "." || base == ".." {
        return None;
    }
    let mut extension = String::new();
    for index in 8..11 {
        let byte = entry[index];
        if byte == b' ' {
            break;
        }
        extension.push(byte as char);
    }
    let lower = entry[12];
    if lower & 0x08 != 0 {
        base = base.to_lowercase();
    }
    if lower & 0x10 != 0 {
        extension = extension.to_lowercase();
    }
    if extension.is_empty() {
        Some(base)
    } else {
        Some(alloc::format!("{}.{}", base, extension))
    }
}

fn split_parent(path: &str) -> Result<(&str, &str), i64> {
    let trimmed = path.trim_end_matches('/');
    let index = trimmed.rfind('/').ok_or(errno::EINVAL)?;
    let parent = if index == 0 { "/" } else { &trimmed[..index] };
    let name = &trimmed[index + 1..];
    if name.is_empty() || name == "." || name == ".." || name.len() > 64 {
        return Err(errno::EINVAL);
    }
    Ok((parent, name))
}

fn valid_83(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'$' | b'%'
                | b'\''
                | b'-'
                | b'_'
                | b'@'
                | b'~'
                | b'!'
                | b'('
                | b')'
                | b'{'
                | b'}'
                | b'^'
                | b'#'
                | b'&'
        )
}

/// Convert a single path component to an 8.3 short name. Longer names are
/// rejected for now; the UI uses compatible names until LFN support lands.
fn make_83(name: &str) -> Result<[u8; 11], i64> {
    if name.is_empty() || name.len() > 12 {
        return Err(errno::ENAMETOOLONG);
    }
    let (base, extension) = match name.rfind('.') {
        Some(index) => (&name[..index], &name[index + 1..]),
        None => (name, ""),
    };
    if base.is_empty() || base.len() > 8 || extension.len() > 3 {
        return Err(errno::ENAMETOOLONG);
    }
    let mut raw = [b' '; 11];
    for (index, byte) in base.bytes().enumerate() {
        if !valid_83(byte) {
            return Err(errno::EINVAL);
        }
        raw[index] = byte.to_ascii_uppercase();
    }
    for (index, byte) in extension.bytes().enumerate() {
        if !valid_83(byte) {
            return Err(errno::EINVAL);
        }
        raw[8 + index] = byte.to_ascii_uppercase();
    }
    Ok(raw)
}

fn dir_entry(name: &[u8], cluster: u32, size: u32) -> [u8; 32] {
    let mut entry = [0u8; 32];
    let mut raw = [b' '; 11];
    for (index, byte) in name.iter().take(11).enumerate() {
        raw[index] = *byte;
    }
    entry[0..11].copy_from_slice(&raw);
    entry[11] = ATTR_DIR;
    entry[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
    entry[26..28].copy_from_slice(&((cluster & 0xFFFF) as u16).to_le_bytes());
    entry[28..32].copy_from_slice(&size.to_le_bytes());
    entry
}

pub fn open(path: &str, flags: u32) -> Result<u64, i64> {
    with_fat(|fat| fat.open_path(path, flags))
}

pub fn mkdir(path: &str) -> Result<u64, i64> {
    with_fat(|fat| fat.mkdir_path(path))
}

pub fn unlink(path: &str) -> Result<(), i64> {
    with_fat(|fat| fat.unlink_path(path))
}

pub fn rename(old_path: &str, new_path: &str) -> Result<(), i64> {
    with_fat(|fat| fat.rename_path(old_path, new_path))
}

pub fn write(inode: u64, offset: u64, data: &[u8]) -> Result<usize, i64> {
    with_fat(|fat| fat.write_inode(inode, offset, data))
}

pub fn truncate(inode: u64) -> Result<(), i64> {
    with_fat(|fat| fat.truncate_inode(inode))
}

/// Resolve a path relative to the FAT root. Returns the native inode.
pub fn lookup(path: &str) -> Result<u64, i64> {
    with_fat(|fat| fat.lookup(path))
}

pub fn stat(inode: u64) -> Result<aster_abi::FileStat, i64> {
    with_fat(|fat| {
        let location = fat.location(inode)?;
        Ok(aster_abi::FileStat {
            size: location.size as u64,
            is_dir: if location.directory { 1 } else { 0 },
            mode: if location.directory { 0o755 } else { 0o644 },
            inode,
        })
    })
}

pub fn size(inode: u64) -> Result<u64, i64> {
    with_fat(|fat| Ok(fat.location(inode)?.size as u64))
}

/// Read up to `length` bytes. The kernel buffer is deliberately capped; the
/// syscall layer returns a short read and user space loops.
pub fn read(inode: u64, offset: u64, length: usize) -> Result<Vec<u8>, i64> {
    const MAX_READ: usize = 64 * 1024;
    with_fat(|fat| {
        let location = fat.location(inode)?;
        if location.directory {
            return Err(errno::EISDIR);
        }
        let size = location.size as u64;
        if offset >= size {
            return Ok(Vec::new());
        }
        let count = ((size - offset) as usize).min(length).min(MAX_READ);
        let mut data = Vec::with_capacity(count);
        let cluster_bytes = fat.cluster_bytes() as u64;
        let mut cluster = location.first_cluster;
        if cluster < 2 && count > 0 {
            return Ok(Vec::new());
        }
        let mut skip = offset / cluster_bytes;
        while skip > 0 {
            cluster = fat.next_cluster(cluster)?.ok_or(errno::EIO)?;
            skip -= 1;
        }
        let mut within = (offset % cluster_bytes) as usize;
        let mut produced = 0usize;
        while produced < count {
            let sector_in_cluster = within / 512;
            let sector_offset = within % 512;
            let lba = fat.cluster_lba(cluster) + sector_in_cluster as u64;
            let mut sector = [0u8; 512];
            fat.read_sector(lba, &mut sector)?;
            let chunk = (512 - sector_offset).min(count - produced);
            data.extend_from_slice(&sector[sector_offset..sector_offset + chunk]);
            produced += chunk;
            within += chunk;
            if within >= cluster_bytes as usize && produced < count {
                cluster = fat.next_cluster(cluster)?.ok_or(errno::EIO)?;
                within -= cluster_bytes as usize;
            }
        }
        Ok(data)
    })
}

pub fn list(inode: u64, offset: usize, maximum: usize) -> Result<Vec<DirItem>, i64> {
    with_fat(|fat| {
        let location = fat.location(inode)?;
        if !location.directory {
            return Err(errno::ENOTDIR);
        }
        let mut items = Vec::new();
        let mut index = 0usize;
        fat.walk_dir(location.first_cluster, |_, entry_offset, entry| {
            let Some(name) = entry_name(entry) else {
                return Ok(false);
            };
            if index < offset {
                index += 1;
                return Ok(false);
            }
            if items.len() >= maximum {
                return Ok(true);
            }
            items.push(DirItem {
                inode: entry_offset + 1,
                kind: if entry[11] & ATTR_DIR != 0 {
                    NodeKind::Dir
                } else {
                    NodeKind::File
                },
                name,
                size: read_bytes(entry, 28, 4) as u64,
            });
            index += 1;
            Ok(false)
        })?;
        Ok(items)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Dir,
}

pub struct DirItem {
    pub inode: u64,
    pub kind: NodeKind,
    pub name: String,
    pub size: u64,
}

pub fn usage() -> Option<Usage> {
    let mut guard = FAT.lock();
    let fat = guard.as_mut()?;
    let free = match fat.free_cache {
        Some(free) => free,
        None => {
            let free = fat.count_free().ok()?;
            fat.free_cache = Some(free);
            free
        }
    };
    let cluster_bytes = fat.cluster_bytes();
    Some(Usage {
        label: fat.label.clone(),
        cluster_bytes,
        total_bytes: fat.clusters as u64 * cluster_bytes as u64,
        free_bytes: free as u64 * cluster_bytes as u64,
    })
}

/// Self-test performed after mount: read a seed file, then exercise the write
/// path in a scratch namespace and clean it up again.
pub fn self_test() -> bool {
    let Ok(inode) = lookup("/WELCOME.TXT") else {
        crate::warn!("fat32: self-test file /WELCOME.TXT not found");
        return false;
    };
    let Ok(data) = read(inode, 0, 128) else {
        return false;
    };
    if !data.starts_with(b"Welcome") {
        return false;
    }

    let flags =
        aster_abi::open_flags::WRITE | aster_abi::open_flags::CREATE | aster_abi::open_flags::TRUNC;
    let payload = b"persistent write test";
    let Ok(file) = open("/SELFTEST.TXT", flags) else {
        crate::warn!("fat32: self-test cannot create /SELFTEST.TXT");
        return false;
    };
    if write(file, 0, payload).is_err() {
        return false;
    }
    match read(file, 0, payload.len()) {
        Ok(stored) if stored == payload => {}
        _ => return false,
    }
    if truncate(file).is_err() || unlink("/SELFTEST.TXT").is_err() {
        return false;
    }

    let _ = unlink("/SELFTEST");
    if mkdir("/SELFTEST").is_err() {
        return false;
    }
    let Ok(child) = open("/SELFTEST/CHILD.TXT", flags) else {
        return false;
    };
    if write(child, 0, b"nested").is_err() {
        return false;
    }
    unlink("/SELFTEST").is_ok()
}
