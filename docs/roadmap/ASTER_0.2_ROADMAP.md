# Aster OS 0.2 - Core Roadmap

Date: 2026-11-20
Branch: `milestone/aster-0.2-core`
Baseline: `v0.1.0-baseline` (`fc8caf3`)
Prerequisite reading: [ASTER_0.2_AUDIT.md](ASTER_0.2_AUDIT.md)

## 1. Mission

Turn the 0.1.0 developer preview into a system with durable storage, real
process boundaries, and a native IPC path, without discarding any working
functionality and without pretending that unfinished features exist.

Non-goals for 0.2:

- Replacing the kernel architecture, bootloader, or desktop.
- Networking, audio, USB, SMP, or GPU acceleration.
- A journaling native filesystem.
- Multi-user login or a permission system (designed, not implemented).

## 2. Versioning and compatibility

- The kernel and desktop versions move to **0.2.0** once M1 lands.
- `SYS_ABI_VERSION` remains 1. New syscalls are additive; existing numbers,
  structures, and return conventions do not change.
- The boot `BootInfo` layout is unchanged.
- The on-disk data volume is versioned by the FAT32 BPB; no Aster-specific
  superblock is added in M1.

## 3. Milestones

### M1 - Persistent home volume (COMPLETE, 2026-11-20)

Goal: user files, Notes, Settings, and the wallpaper choice survive a reboot.

Deliverables:

1. **PCI configuration access** (mechanism #1) and a small device registry.
2. **AHCI block driver** for the q35 SATA controller, polling mode, 48-bit LBA,
   one command slot, with IDENTIFY and read/write sector operations.
3. **Block layer** with sector validation and a bounded cache.
4. **GPT parser** (protective MBR, header CRC, entry array, first data
   partition) and **FAT32 mount** (BPB validation, FAT chain walk, directory
   iteration).
5. **FAT32 read/write**: open, read, write, truncate, create, mkdir, unlink.
   Short 8.3 names first; long names are a stretch goal inside M1.
6. **VFS mount table**: longest-prefix routing, encoded inode numbers, mount at
   `/home/user` when the volume is present.
7. **Desktop integration**: Settings/Notes/Files use the persistent home;
   Settings > Storage reports real capacity, free space, and mount state.
8. **Robustness fixes** required by the new code paths: full user exception
   containment, PMM frame-range check, partial-mapping cleanup, path copy
   without a fixed 256-byte read.
9. **Tests**: host image verifier, kernel FAT self-test, guest write/reboot/read
   test, corrupted-BPB test, missing-disk test, and the full existing suite.
10. **Documentation**: storage architecture, updated status/features/threat
    model, refreshed screenshots where the UI changes.

Acceptance criteria:

- With `build/data.img` attached, the boot log reports the mounted volume, its
  size, and its cluster count.
- A file created in the guest survives a full QEMU restart and is verified by
  the host tools.
- The boot disk is never written by the guest.
- A data image with a corrupted BPB fails to mount, logs a clear error, and the
  desktop still starts; Settings reports "Storage unavailable".
- With no data disk attached, the system boots to the desktop as before.
- All pre-existing tests pass unchanged.
- Every new syscall-free path has a kernel or host acceptance test.

Explicit limitations to document if not solved in M1:

- No journaling or power-loss atomicity.
- 8.3 names only (if LFN is not completed).
- Single mounted volume; no unmount or hotplug.
- No per-user permissions.

M1 result: all acceptance criteria above were executed. The persistence test
wrote `/home/user/PERSIST.TXT` and changed the accent, the independent host FAT
parser verified the volume, and a second boot returned the same bytes and
settings. Missing and corrupted volumes both reached the desktop. Deviations:
the volume uses 8.3 names, transfers one sector per AHCI command, and has no
journal. These are documented in `docs/architecture/STORAGE.md` and in
Settings.

### M2 - Process isolation and lifecycle (current)

Depends on: M1 robustness fixes (exception containment, PMM bounds).

Deliverables:

- Loader hardening: overflow checks, no RWX, entry-in-executable check, overlap
  rejection, segment and stack cleanup on failure.
- `Process` gains `parent_pid`, `state`, and a children list; `SPAWN(path)`
  loads an ELF from the filesystem into a fresh address space.
- `WAIT(pid)` returns the exit status; zombies are reaped only by the parent
  (or by PID 1 for orphans).
- Per-process page and process quotas; `VM_ALLOC` caps and overflow checks.
- Full user exception containment with distinct exit codes.
- Kernel-stack guard pages.
- Acceptance: two independent processes map the same virtual address to
  different frames; a faulting child does not affect its parent; spawn/wait
  returns the exact exit status; repeated spawn/exit for 100 iterations leaves
  PMM accounting stable.

### M3 - IPC channels

Depends on: M2 (processes, wait primitive, handle tables).

Deliverables:

- Kernel channel objects with bounded inline messages, per-process handles,
  blocking send/recv with timeouts, close/hangup semantics.
- `CHANNEL_CREATE` returns a bidirectional endpoint pair; `SPAWN` can seed a
  bootstrap handle into the child.
- Wait queues in the scheduler: `block_on(channel)`/`wake(tid)`, replacing the
  timestamp-only block.
- Acceptance: two user processes exchange 10,000 messages without loss; a
  forged handle returns `EBADF`; a full queue blocks and drains; close wakes
  waiters with `EPIPE`.

### M4 - First system service

Depends on: M3 and M1.

Candidate: a storage service that owns the block device and FAT volume, with
the desktop mounting it through IPC. The kernel filesystem stays as a fallback
until the service passes the same persistence tests. Acceptance: the desktop
survives a service restart; all M1 persistence tests pass through IPC.

### M5 - Security and robustness

- SMEP, SMAP, UMIP (with CR4 feature detection).
- Capability checks for `SHUTDOWN`, raw block access, and service endpoints.
- Kernel log audit trail with process identity.
- Fuzz the ELF loader and FAT parser with corrupted images (host-side corpus and
  in-guest probes).
- Update the threat model with verified evidence only.

### M6 - Performance

- Damage tracking in the compositor (idle desktop below 5% of one core).
- Lazy kernel-heap growth and demand-paged desktop heap.
- Frame cache and fewer full-screen copies.
- Re-measure with `tools/bench.py` and record before/after.

## 4. M1 architecture

```
applications (ring 3)
        |
   syscalls (OPEN/READ/WRITE/READDIR/STAT/...)
        |
   fs.rs  (VFS front-end: path routing + inode dispatch)
        |                         |
   ramfs (mount 0)          FAT32 (mount 1)
                                   |
                              block layer
                                   |
                              AHCI driver
                                   |
                          QEMU data disk (GPT/FAT32)
```

### 4.1 Disk layout

- Boot disk: unchanged, `if=ide,index=0`, read-only from the kernel's point of
  view.
- Data disk: new, `if=ide,index=1`, 512 MiB raw image, GPT with one FAT32
  partition, 4 KiB clusters, built by `tools/image.py`.
- The kernel locates the data disk by AHCI port presence and reads LBA 0.

### 4.2 Inode encoding

The ABI passes inodes as `u64`. The VFS reserves the top byte for the mount:

- `0x00...`: ramfs inode (existing behaviour).
- `0x01...`: FAT32 inode, low bits identify the directory entry by its byte
  offset within the volume; the FAT root has a reserved value.

This keeps the syscall ABI and the desktop unchanged.

### 4.3 Write policy

- Sector writes are synchronous and validated (LBA range, buffer length).
- FAT updates write both FAT copies.
- File data is written before the directory entry size/cluster is committed.
- No journal: a crash mid-write may leave a truncated file, but the volume
  remains mountable. This is documented, not hidden.

### 4.4 Failure policy

- Missing disk, bad GPT, bad BPB, or unsupported geometry: log once, do not
  mount, continue boot. `/home/user` falls back to ramfs.
- I/O timeout: return `EIO`; never panic on device errors.
- Corrupted chains: stop at the first invalid cluster, return the bytes read so
  far or `EIO`, and never loop unbounded.

## 5. Testing strategy

| Layer | Test |
|---|---|
| Host | Extend `tests/test_image.py` to build and verify a data image |
| Kernel | AHCI IDENTIFY + sector read self-test at boot (non-fatal if absent) |
| Kernel | FAT32 mount, create/write/read/truncate/unlink self-test on a scratch directory |
| Guest | Write `/home/user/PERSIST.TXT`, power off, boot again, verify content |
| Guest | Corrupt BPB image: mount fails, desktop starts, Settings says unavailable |
| Guest | No data disk: desktop starts, storage page says unavailable |
| Regression | `tools/test.py`, `tools/ui_test.py`, `tools/pointer_test.py`, `tools/app_probe.py`, `tools/wm_probe.py` |

Tests that write to the data disk use `build/data-test.img`; the interactive
`build/data.img` is only touched by the explicit persistence test.

## 6. Definition of done for M1

1. Code builds with no new warnings that indicate real bugs.
2. All acceptance tests above have been executed in this environment.
3. `docs/architecture/STORAGE.md` describes the shipped design and limits.
4. `docs/PROJECT_STATUS.md`, `docs/FEATURES.md`, README, and CHANGELOG state the
   real persistence status.
5. The repository is committed on `milestone/aster-0.2-core` with the baseline
   tag intact.
