# Aster OS 0.3.0 - Roadmap

Date: 2026-11-20
Baseline: `v0.2.0-baseline` (`68f499a`)
Branch: `milestone/aster-0.3.0`

Mission: make persistence, storage architecture, services and desktop
integration more reliable and more useful, without redesigning the working
desktop or faking unsupported hardware.

Status: M1-M5 delivered and tested. See TEST_REPORT.md, PERFORMANCE.md and
KNOWN_LIMITATIONS.md for evidence and gaps.

## M1 - Storage architecture

- `block.rs`: device registry with capacity, sector size, bounds checks, read,
  write, flush and I/O counters. AHCI implements the backend.
- AHCI FLUSH CACHE EXT support and device model/port reporting.
- `mount.rs`: mount table, longest-prefix routing, filesystem kind, device and
  partition identity; root ramfs plus the FAT32 home.
- `fs.rs` routes through the mount table instead of hardcoding `/home/user`.
- New `STORAGE_DETAILS` syscall: device, filesystem, capacity, used/free,
  partition range, sector/cluster size, I/O counters, label.
- Block/mount boot self-tests: bounds rejection, flush, route resolution.

Acceptance: Settings shows real device/filesystem/capacity/used data; invalid
LBA access returns an error; the host verifier still reads a guest-written
volume.

## M2 - Filesystem reliability and settings

- `RENAME` syscall for ramfs and FAT32 (same-directory rename).
- Open flag validation, consistent error codes, `SYNC` syscall and AHCI flush.
- Versioned settings format (`version=1`), per-key validation and clamping,
  defaults, corruption recovery (`SETTINGS.BAK`), atomic replace
  (`SETTINGS.NEW` + rename + sync).
- Filesystem self-tests for rename, duplicate names, invalid paths and
  disk-full/error propagation.

Acceptance: corrupt settings recover to defaults without a crash; a valid v1
file round-trips; data survives a reboot after `SYNC`; rename works on both
filesystems.

## M3 - Services (COMPLETE)

- `configd`: a real user-space settings service over IPC, with GET/SET/PING and
  graceful exit.
- Desktop service manager: start, state, request/response, failure detection,
  restart and logs; fallback to direct settings I/O when unavailable.
- IPC receive timeout (`ETIMEDOUT`) so a service failure cannot hang the shell.
- Terminal `services` diagnostics; `service_test.py`.

Acceptance: configd serves settings; killing it triggers a restart; the desktop
remains usable and settings still save through the service afterwards.

## M4 - Desktop integration (COMPLETE)

- Files: forward/back, breadcrumbs, new folder, rename, delete with confirm,
  copy/move/paste, file info, sorting, search, Downloads/Desktop locations,
  empty and error states. All operations use real syscalls.
- Settings: storage details (device, filesystem, used/free) and settings
  service state.
- Monitor: storage bar, I/O counters, kernel log view from `LOG_READ`.
- `LOG_READ` syscall and a bounded, non-blocking log view.

Acceptance: UI probes exercise the Files operations and Monitor pages; no
control is inert.

## M5 - Verification and documentation (COMPLETE)

- Tests: block bounds, mount routing, rename, settings corruption/round-trip,
  service restart, Files operations, full regression.
- Performance: boot, storage init, file read/write throughput, frame time,
  service overhead.
- Release docs: TEST_REPORT, PERFORMANCE, STORAGE, SERVICES,
  KNOWN_LIMITATIONS; update README/status/features.

## Out of scope

- Full audio, networking, USB, NVMe, SMP, installer, package manager.
- VFAT long names and journaling (documented; a larger filesystem milestone).
- Moving all nine apps out of the desktop process.

## Risks

| Risk | Mitigation |
|---|---|
| Mount refactor breaks the desktop home | Keep `/home/user` routing and run all existing tests after each step |
| Atomic rename on FAT is not journaled | Temp + backup + rename + flush; document the guarantee as "best effort, not crash-atomic" |
| Blocking service calls freeze the UI | IPC timeout and fallback to direct settings I/O |
| Files copy recursion damages data | Same-parent rename for rename; copy to a new name, never overwrite silently; confirmation for delete |
| Benchmarks skewed by dirty settings | Deterministic volumes from `tools/seeds.py` |
