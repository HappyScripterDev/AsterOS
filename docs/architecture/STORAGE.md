# Persistent Storage

Status: **0.3.0 update** (2026-11-20). The home volume is a real FAT32 volume
on a separate QEMU disk, reached through a block registry and a mount table.
Settings are versioned and backed up. Files written by a guest survive a
reboot; this is verified by an independent host-side parser in
`tools/persistence_test.py`.

0.3.0 added: `block.rs` (capacity/bounds/flush/counters), `mount.rs`
(longest-prefix mount table), AHCI FLUSH CACHE EXT, `STORAGE_DETAILS`,
`RENAME`, `SYNC`, same-directory FAT rename, and settings v1 with backup and
corruption recovery.

## 1. Scope

M1 provides:

- PCI configuration access and AHCI disk I/O for the q35 reference machine.
- A block registry with sector-range validation, flush, counters and error
  propagation.
- GPT parsing with header CRC verification.
- A FAT32 driver with mount, lookup, read, write, create, mkdir, truncate and
  unlink, including directory growth and FAT-chain allocation.
- A VFS mount at `/home/user` backed by the volume.
- `STORAGE_INFO` and `STORAGE_DETAILS` syscalls plus a real Settings > Storage
  page (device, filesystem, used/free, partition, I/O).
- Acceptance tests for write/reboot/read, missing disks and corrupt volumes.

Not yet: long file names, journaling, permissions, multiple volumes, unmount,
hotplug, a storage service, and non-AHCI hardware. Rename is same-directory
only; Files moves by copy+delete.

## 2. Hardware path

```
q35
 └─ ICH9 SATA controller (8086:2922, PCI 00:1f.2)
     ├─ port 0: boot disk  build/aster.img (never written by the kernel)
     └─ port 1: data disk  build/data.img  (GPT + one FAT32 partition)
```

- `kernel/src/drivers/pci.rs` implements configuration mechanism #1
  (`0xCF8/0xCFC`), finds the AHCI controller by class `0x01/0x06/0x01`, reads
  BAR5, and enables memory decoding plus bus mastering.
- `kernel/src/mm/vmm.rs::map_kernel_mmio` maps the 8 KiB HBA window at a
  dedicated kernel virtual address. The kernel's upper-half PML4 entries are
  copied into every process, so ring 0 can reach the controller from any
  address space.
- `kernel/src/drivers/ahci.rs` is a polling driver: global HBA enable, per-port
  reset/start, one command slot, IDENTIFY, and single-sector READ/WRITE DMA EXT
  commands. Command list, FIS receive area, command table and DMA buffer are
  one physical frame each, accessed through the direct map. Timeouts and task
  file errors are reported, never ignored.
- The driver deliberately transfers one 512-byte sector per command because the
  PMM has no physically-contiguous allocation API. This is slower but correct;
  multi-sector DMA is a follow-up once a contiguous-frame allocator exists.

## 3. Partitioning

`kernel/src/storage/gpt.rs`:

1. Reads LBA 0, checks the protective MBR signature.
2. Reads LBA 1, checks `EFI PART`, the header size, and the header CRC32 with
   the CRC field zeroed.
3. Bounds-checks the entry array against the device size.
4. Selects the first partition whose type GUID is the basic-data GUID used by
   the Aster data image (`EBD0A0A2-...-68B6B72699C7`).

The boot ESP has a different type GUID and is never selected. A disk with no
data partition is logged and skipped.

## 4. FAT32 driver

`kernel/src/storage/fat32.rs`:

| Item | Implementation |
|---|---|
| Mount | Validates the boot signature, 512-byte sectors, power-of-two cluster size, FAT count, FAT32 geometry, root cluster and cluster count |
| Cluster size | 4096 bytes (8 sectors), 130,552 clusters on the 512 MiB volume |
| FAT access | On-demand sector reads; writes update **both** FAT copies |
| Allocation | First free entry from a rotating hint, wrap-around scan, zeroed cluster |
| Directories | Walks cluster chains, skips LFN/volume entries, supports growth by allocating a new cluster |
| Names | 8.3 uppercase, case-insensitive lookup; longer names are rejected with `ENAMETOOLONG` |
| Files | Read at byte offsets, write with read-modify-write sectors, sparse-gap zeroing, 4 MiB cap |
| Inodes | Byte offset of the directory entry within the volume, plus the mount tag |
| Timestamps | Written from the CMOS RTC in DOS format |

The driver holds one interrupt-safe lock. Every operation performs synchronous
disk I/O. A sector error returns `EIO`; a malformed chain returns `EIO`; no
path panics on device errors.

## 5. VFS integration

`kernel/src/fs.rs` keeps the existing path-based ABI and adds a mount prefix:

- `mount::resolve` picks the longest matching mount prefix; `/home/user`
  routes to the FAT driver and every other path remains on the in-memory ramfs.
- The mount entry records the device, partition start/size, label and read-only
  flag. `fs.rs` no longer hardcodes the home path.
- Inodes use the top byte as a mount tag (`0x01...` = FAT32), so
  `read`/`write`/`stat`/`list` dispatch by inode without changing the ABI.
- Directory listings return tagged inodes, and `FileStat.inode` is tagged.
- When no data disk is present, the mount is absent and `/home/user` falls back
  to the ramfs seeds. The routing check is explicit, so no path is sent to an
  unmounted driver.

`fs::init` creates the `/home/user` mount placeholder but skips the home seed
files when the FAT volume is mounted.

## 6. Write ordering and durability

For a file write:

1. Allocate and zero clusters as needed, updating both FAT copies.
2. Write file data sectors (read-modify-write for partial sectors).
3. Update the directory entry's first cluster and size **last**.

For create, the directory entry is written after the directory cluster (if any)
has been initialised. For unlink, the entry is marked `0xE5` and its chain is
freed.

Writes are synchronous: when a syscall returns success, the bytes have been
sent to the AHCI device. There is no guest cache to flush. There is also **no
journal or checksum**, so a power loss during a multi-sector operation can leave
a truncated file or a leaked cluster. The volume remains mountable in the tested
corruption cases because malformed chains and geometry are rejected.

## 7. Errors and limits

| Condition | Result |
|---|---|
| No SATA controller | Boot continues, persistence disabled |
| No data partition | Disk skipped, boot continues |
| Bad GPT header CRC | Disk skipped, warning logged |
| Bad FAT boot sector | Mount fails, `/home/user` stays on ramfs |
| I/O timeout / task file error | `EIO` to the caller |
| Cluster chain loop or out-of-range | `EIO`, bounded walk |
| Volume full | `ENOSPC` |
| Name longer than 8.3 | `ENAMETOOLONG` |
| File above 4 MiB | `ENOSPC` |

## 8. Security

- The boot disk is only ever read. User data lives on a separate disk, so a
  filesystem bug cannot overwrite the kernel or desktop image.
- AHCI DMA targets dedicated frames allocated by the PMM; no user pointer is
  ever programmed into a PRDT.
- The HBA BAR is mapped read/write, NX, and only in the kernel half. User page
  tables inherit the kernel entry but not user permissions.
- All path and buffer arguments still pass through the existing user-pointer
  validation. `PTE_OWNED` marks anonymous frames so address-space teardown
  cannot free device MMIO or shared mappings.
- No permissions or ownership exist yet; every process can read and write the
  whole home volume. This is documented, not implied.

## 9. Tests

| Test | What it proves |
|---|---|
| `fat32::self_test` at boot | Read a seed, create/write/read/truncate/unlink a file, mkdir and recursively unlink a directory |
| `tools/persistence_test.py` | Boot 1 writes a file and changes the accent; host verifier parses the FAT volume; boot 2 reads both back; host verifier runs again |
| `tools/storage_probe.py` | Settings > Storage shows real capacity; no-data boot reaches the desktop; a zeroed FAT boot sector is rejected without a fault |
| `tools/test.py` | Boot markers include the mount and FAT self-test |
| `tools/test_image.py` | Host builder/verifier round-trip and GPT CRC |

## 10. Follow-ups

1. VFAT long file names (read and write) and case preservation.
2. Multi-sector DMA with a contiguous-frame allocator; sector cache.
3. Journal or copy-on-write metadata for crash consistency.
4. Per-process file permissions and a storage service over IPC (M4).
5. AHCI interrupt mode instead of polling.
6. Unmount, multiple volumes, and NVMe/AHCI hotplug.
