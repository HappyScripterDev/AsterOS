//! GPT parsing: protective MBR, header CRC, entry array, data partition.

use crate::block;

const DATA_TYPE_GUID: [u8; 16] = [
    0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7,
];

pub struct Partition {
    pub start: u64,
    pub sectors: u64,
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn read(disk: usize, lba: u64) -> Result<[u8; 512], &'static str> {
    let mut sector = [0u8; 512];
    block::read(disk, lba, &mut sector).map_err(|_| "block read failed")?;
    Ok(sector)
}

/// Find the first partition carrying the Aster data type GUID.
pub fn find_partition(disk: usize) -> Result<Option<Partition>, &'static str> {
    let sector_count = block::info(disk).map(|device| device.sectors).unwrap_or(0);
    if sector_count < 34 {
        return Err("disk too small");
    }
    let mbr = read(disk, 0)?;
    if mbr[510] != 0x55 || mbr[511] != 0xAA {
        return Err("missing MBR signature");
    }

    let header = read(disk, 1)?;
    if &header[0..8] != b"EFI PART" {
        return Err("missing GPT header");
    }
    let header_size = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
    if !(92..=512).contains(&header_size) {
        return Err("invalid GPT header size");
    }
    let stored_crc = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let mut check = [0u8; 512];
    check[..header_size].copy_from_slice(&header[..header_size]);
    check[16..20].fill(0);
    if crc32(&check[..header_size]) != stored_crc {
        return Err("GPT header CRC mismatch");
    }

    let entries_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
    let entry_count = u32::from_le_bytes(header[80..84].try_into().unwrap()) as u64;
    let entry_size = u32::from_le_bytes(header[84..88].try_into().unwrap()) as u64;
    if entry_size < 128 || entry_size > 512 {
        return Err("unsupported GPT entry size");
    }
    if entries_lba == 0
        || entry_count == 0
        || entries_lba
            .checked_add((entry_count * entry_size + 511) / 512)
            .map(|end| end > sector_count)
            .unwrap_or(true)
    {
        return Err("GPT entry array outside the disk");
    }

    let per_sector = 512 / entry_size as usize;
    let sectors = (entry_count as usize + per_sector - 1) / per_sector;
    for index in 0..sectors {
        let sector = read(disk, entries_lba + index as u64)?;
        for slot in 0..per_sector {
            let offset = slot * entry_size as usize;
            let entry = &sector[offset..offset + entry_size as usize];
            if entry[0..16].iter().all(|byte| *byte == 0) {
                continue;
            }
            if entry[0..16] == DATA_TYPE_GUID {
                let first = u64::from_le_bytes(entry[32..40].try_into().unwrap());
                let last = u64::from_le_bytes(entry[40..48].try_into().unwrap());
                if last < first || last >= sector_count {
                    return Err("GPT partition range is invalid");
                }
                return Ok(Some(Partition {
                    start: first,
                    sectors: last - first + 1,
                }));
            }
        }
    }
    Ok(None)
}
