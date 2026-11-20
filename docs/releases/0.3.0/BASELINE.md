# Aster OS 0.3.0 - Baseline Audit

Date: 2026-11-20
Branch: `milestone/aster-0.3.0`
Baseline tag: `v0.2.0-baseline` (`68f499a`)
Previous release: `v0.2.0-core`

This document records the **verified** state at the start of 0.3.0. Claims were
checked against the code and by running the tests, not inferred from older
roadmaps.

## 1. Verification runs

| Check | Result |
|---|---|
| `python tools/build.py` | PASS, boot image + 512 MiB data image |
| `python tools/test.py` | PASS, all host tests and the boot smoke test |
| `python tools/isolation_test.py` | PASS: spawn status 42, fault status -11, IPC pong, forged handle -3, hangup -14, bad syscalls -5 |
| `python tools/persistence_test.py` | PASS: guest writes, host FAT verifier, reboot, guest reads back |
| `python tools/storage_probe.py` | PASS: real capacity, missing disk, corrupt volume |
| `python tools/ui_test.py` | PASS |
| `python tools/pointer_test.py`, `app_probe.py`, `wm_probe.py` | PASS |
| Benchmark (clean volume) | first frame 4.75 s, steady 47.2 ms, 397 MiB free |

## 2. What exists

### Boot and kernel

- Custom UEFI loader, GPT/FAT32 ESP, higher-half kernel, GDT/TSS/IDT.
- PIC/PIT, preemptive single-CPU scheduler, ring-3 processes, 32 syscall
  numbers (31 implemented; `SET_THEME` returns `ENOSYS` because appearance is
  fixed to light).
- PMM with a 32 GiB bitmap, 4-level paging per process, NX, W^X, CR0.WP,
  EFER.NXE, user/write-checked copy helpers, `PTE_OWNED` frame ownership.
- Kernel heap: first-fit, 1 MiB growth chunks, coalescing, 1 MiB allocation
  tested at boot.
- User exceptions are all contained; a child writing to kernel memory exits -11
  while the desktop survives.

### Processes and IPC

- Private PML4 per process, 64 KiB NX stack, ELF loader hardened against
  overflow, RWX, bad entry points and overlapping segments.
- `SPAWN`, `WAIT`, zombies, orphan reaping, exit status through a user pointer.
- Channels: two endpoints per channel, per-process handles 3..15, 8 x 256 B
  queues, blocking send/receive, close/hangup, bootstrap handle in `SPAWN`.

### Storage

- PCI config (mechanism #1), polling AHCI driver for q35, IDENTIFY,
  single-sector DMA reads/writes. **No flush command and no block abstraction.**
- GPT parsing with header CRC, basic-data partition lookup.
- FAT32: mount, 8.3 names, read/write/create/mkdir/truncate/recursive unlink,
  both FAT copies, directory growth, 4 MiB file cap, 64 KiB read chunks.
- The VFS hardcodes `/home/user` routing in `fs.rs`; there is no mount table,
  filesystem instance object or unmount lifecycle.
- Boot disk is never written; the data disk is separate.

### Desktop

- Light macOS-inspired shell, menu bar, Spotlight, dock, Control Centre,
  notifications, virtual desktops, nine in-process applications.
- Settings currently stored as a flat `SETTINGS.CNF` (`theme`, `accent`,
  `animations`, `wallpaper`, `reduce_motion`, `brightness`) with no version,
  no validation of unknown values and no corruption recovery.
- Files has Home/Documents/Notes/System/Logs/Root, back navigation, list view,
  double-click open, but **no forward, rename, delete, copy, move, info,
  sorting, search, Downloads/Desktop or breadcrumbs**.
- Monitor shows memory graphs, process count and process list; no storage or
  logs.
- Settings > Storage shows mount state, total and free space from
  `STORAGE_INFO`.
- Logging: serial plus a 64 KiB kernel ring with levels, but no user-space read
  syscall and no dedicated log UI.

### Tests

Host image verifier; boot smoke; UI/app/pointer/window probes; persistence
(host verifier + reboot); storage (capacity, missing disk, corrupt volume);
isolation (spawn/wait/fault/IPC/handles/syscall validation); benchmark with a
clean volume.

## 3. Verified deficiencies for 0.3.0

1. **No block layer.** FAT/AHCI talk directly; no device objects, capacity API,
   flush, I/O counters or bounds-checked central access.
2. **No mount architecture.** Routing is a hardcoded prefix; no mount table,
   filesystem identity or lifecycle.
3. **Storage information is shallow.** Settings cannot show device, filesystem,
   used space or I/O activity.
4. **Settings are fragile.** No version, no range validation, no backup,
   no atomic replacement, no corruption recovery; a torn write can lose all
   preferences silently.
5. **No service framework.** No configd/storage service, no supervisor, no
   restart policy, no service logging.
6. **Files is read-only in practice.** No rename/delete/copy/move/info/sort/
   search/forward/Downloads/Desktop/breadcrumbs.
7. **Monitor has no storage or log view.** `read_log` exists in the kernel but
   is not exposed.
8. **FAT32 limits.** 8.3 names, no journal, single sector per AHCI command,
   no rename.
9. **Block I/O has no flush.** Durability depends on QEMU's writeback timing.
10. **No per-process quotas/guard pages/ASLR/SMEP/SMAP** (carried from 0.2.0).

## 4. Constraints

- Preserve the light desktop and all nine applications.
- Never write the boot ESP from the kernel.
- Keep the 0.2.0 tests passing; do not weaken them.
- New syscalls are additive; do not renumber.
- Favour correctness and honest failure over new cosmetic features.
- The existing FAT32 volume must remain readable by the host verifier.
