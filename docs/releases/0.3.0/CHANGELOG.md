# Aster OS 0.3.0 - Changelog

Status: released 2026-11-20.

## Storage

- Added a block-device registry with capacity, bounds-checked sector I/O,
  flush and counters; AHCI now supports FLUSH CACHE EXT.
- Added a mount table with longest-prefix routing, device/partition identity
  and root/FAT32 instances. The VFS no longer hardcodes `/home/user`.
- Added `STORAGE_DETAILS` for device, filesystem, partition, capacity,
  used/free space and I/O counters.

## Filesystem and settings

- Added `RENAME` and `SYNC` syscalls and FAT32 same-directory rename.
- Settings are now versioned, validated and written through a temporary file
  with a backup; corrupt settings recover to defaults.
- Added `LOG_READ` for user-space log inspection.
- AHCI flush and explicit sync after settings saves.

## Services and desktop

- Added the `configd` settings service and a desktop service manager with
  restart and direct-I/O fallback.
- Added an IPC receive timeout for service calls.
- Files: forward navigation, breadcrumbs, new folder, rename, delete, copy,
  move, info, sorting, search and new sidebar locations.
- Settings: real storage details and service state.
- Monitor: storage usage, I/O counters and a kernel log view.

## Tests and documentation

- Storage block/mount self-tests, settings corruption and rename tests,
  service restart test, Files operation probe.
- Release baseline, roadmap, test report, performance, storage, services and
  known-limitations documents.
