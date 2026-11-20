# Aster OS 0.3.0 - Storage

## 1. Layers

```
VFS (fs.rs)
  └─ mount table (mount.rs)          longest-prefix routing
       ├─ ramfs  "/"                  system files, volatile
       └─ FAT32  "/home/user"         persistent home
            └─ fat32.rs               files, directories, allocation
                 └─ block.rs          device registry, bounds, counters
                      └─ drivers/ahci.rs   q35 SATA, polling DMA
```

### Block layer

`block.rs` exposes one device object per attached ATA disk:

- port, model, sector count, 512-byte sectors;
- `read`, `write`, `flush` with `lba < sectors` enforcement;
- normalised `EINVAL`/`EIO`, plus error accounting;
- counters: reads, writes, sectors read/written, flushes, errors.

`block::self_test` at boot reads LBA 0, rejects the first out-of-range LBA,
and executes a flush. AHCI implements `FLUSH CACHE EXT` (0xEA) with no PRDT.
The boot disk is never written by the kernel.

### Mount table

`mount.rs` stores mount point, filesystem kind, device, partition start and
size, label and read-only flag. `resolve` picks the longest matching prefix at
a component boundary. `fs.rs` slices the path and dispatches to ramfs or FAT32;
it no longer hardcodes `/home/user`.

`detach` exists for internal lifecycle work but is not exposed as a syscall:
there is no reference-counted busy check for open files yet. Root cannot be
detached.

### Storage details

`STORAGE_DETAILS` (syscall 33) returns: mount/health state, filesystem kind,
device index, sector and cluster size, total/free bytes, device sectors,
partition start/sectors, I/O counters and the volume label. Settings renders
this data; Monitor shows used percentage and free MiB.

## 2. FAT32 behaviour

- 8.3 uppercase names, case-insensitive lookup.
- 4096-byte clusters, 130,552 clusters, both FAT copies updated.
- Create, read, write, mkdir, truncate, recursive unlink, same-directory
  rename, directory growth.
- Write ordering: allocate/zero clusters → write data → update the directory
  entry size/cluster last.
- `WRITE_FD` is chunked; multi-sector data is now copied with the correct
  source offset (a 64 KiB pattern test verifies every byte).
- `RENAME` (syscall 34) changes the directory entry name only; cross-directory
  moves are rejected. Files uses copy+delete for moves.
- `SYNC` (syscall 36) flushes the data device. configd and the direct settings
  fallback call it around settings replacement.
- A 64 KiB file writes in ~19 ms and reads in ~17 ms under QEMU TCG.

## 3. Settings file

`/home/user/SETTINGS.CNF` is versioned:

```
version=1
accent=...
animations=...
wallpaper=...
reduce_motion=...
brightness=...
```

configd owns it in the normal path; the desktop writes directly only if the
service is unavailable. Both paths write `SETTINGS.NEW`, flush, keep
`SETTINGS.BAK`, rename into place and flush again. Invalid values fall back per
key and are rewritten; structurally corrupt files are preserved as
`SETTINGS.BAD` and replaced with defaults. Legacy 0.2 files without a version
line are migrated, not destroyed.

## 4. Failure behaviour

| Failure | Behaviour |
|---|---|
| No SATA controller | Boot continues, no mount, memory-only home |
| No data partition | Disk skipped, warning |
| Bad GPT CRC | Disk skipped, warning |
| Bad FAT boot sector | Mount rejected, desktop continues |
| Out-of-range LBA | `EINVAL`, error counter increments |
| Disk I/O error | `EIO` to the filesystem caller |
| Chain loop / bad cluster | `EIO`, bounded walk |
| Disk full | `ENOSPC` |
| 8.3 name rejected | `ENAMETOOLONG` |
| Corrupt settings | `SETTINGS.BAD` + defaults |
| Valid but invalid values | defaults for those keys, file rewritten |

## 5. Limits

- No journal, checksums or crash-atomic multi-sector guarantee.
- 8.3 names only; no VFAT long names.
- One data disk, one FAT32 mount; no unmount/hotplug.
- Single-sector DMA and no FAT cache.
- No permissions: the volume is world-readable/writable for the single user.
