# Aster OS 0.7.0 - Storage Foundation (AFS v2)

Status: planned. Baseline: 0.6.0 platform work (untagged, 2026-11-22).

## Product direction

- 0.7.0 replaces FAT32 as the persistent home with AFS v2: a real filesystem
  with long names, permissions, checksums, journaling and crash recovery.
- This is the prerequisite for 0.7.1 users/login/permissions and for the 1.0
  crash-consistency gate. Without it, "permissions" would be a metadata patch.
- The UI remains the light glass desktop from 0.5. No visual rewrite.
- No fake encryption, snapshots or "secure" badges. If a feature is not
  implemented, the docs say so.

## Goals

1. Design and implement the AFS v2 on-disk format and a host-side formatter,
   inspector and verifier.
2. Implement the kernel driver: mount, inode and block allocation, directories,
   file I/O, rename, unlink, truncate, fsync, stat and readdir.
3. Make metadata updates crash-consistent through a journal and recovery, with
   fsck and crash-injection tests.
4. Migrate `/home/user` from FAT32 to AFS without data loss and keep the FAT32
   volume as a read-only backup.
5. Add UID/GID/mode fields and kernel enforcement, ready for 0.7.1 accounts.
6. Update the VFS, ABI, apps, tests and documentation.

## Non-goals

- Users, login, lock screen, password hashing and sessions (0.7.1).
- Capabilities, sandboxing, ASLR/KASLR, SMEP/SMAP/UMIP, quotas and audit log
  (0.7.2).
- CSPRNG, TLS, certificates, signatures, secure boot and firewall (0.7.3).
- Encryption at rest, compression, deduplication and network filesystems.
- Full POSIX ACLs. Start with UID/GID/mode; ACLs can follow in 0.7.1.
- Moving the root filesystem off ramfs. AFS mounts `/home/user` first.

## Starting point

- `milestone/aster-0.6.0-platform` is the current tip. The 0.6.0 tag is not set
  and must not be set retroactively.
- AHCI is interrupt-driven with a polling fallback; the block registry owns
  capacity checks, bounds, flush and counters.
- `kernel/src/storage/fat32.rs` provides the current home; long names are
  rejected and there is no journal.
- The VFS routes by longest mount prefix; inodes use a mount tag.
- Permissions do not exist; every process can read and write the whole home.
- `tools/persistence_test.py` verifies FAT32 with an independent host parser.
- USB, audio, Bluetooth, SMP, HiDPI and VirtIO-GPU remain outside this release.

## Phase 0 - Audit and baseline

- Create `milestone/aster-0.7.0-afs` from the 0.6 tip.
- Build and run the full suite; fix any 0.6 integration breakage first.
- Record the current storage path: block registry, AHCI mode, FAT32 limits,
  VFS inode encoding, mount table, persistence tests, Files/Terminal behavior.
- Measure baseline: sequential and random read/write, fsync, mount time,
  migration source size and boot time.
- Write `BASELINE.md`.

Gate: build green, existing tests green, baseline documented.

## Phase 1 - AFS v2 format and host tooling

- Write `docs/architecture/AFS.md` with the on-disk layout before coding.
- Proposed layout:
  - 4 KiB blocks, little-endian.
  - Superblock: magic `AFS2`, version, geometry, block size, inode table start
    and count, bitmap start, journal start and length, root inode, feature
    flags and a CRC32 checksum.
  - Inodes: type, mode, UID, GID, size, timestamps, direct extents, one indirect
    extent block and a checksum. Choose a fixed inode size and document it.
  - Directories: inode, type, name length and UTF-8 name; checksum each block.
  - Allocation bitmap: one bit per block; reserved ranges for superblock,
    inode table, bitmap and journal.
  - Journal: write-ahead metadata log with transaction records, commit marker
    and replay; checkpoint after commit.
- Reuse the existing CRC32 implementation; do not add a crypto dependency.
- Write `tools/afs_tool.py` to format, inspect and verify a raw image. It must
  be deterministic and usable by host tests.
- Gate: host tests format an image, create/read/write/rename/unlink files and
  directories, store long UTF-8 names and detect a corrupted checksum.

## Phase 2 - Kernel driver

- Add `kernel/src/storage/afs.rs` and submodules as needed.
- Integrate with the block registry and mount table; add `FsKind::Afs`.
- Implement mount validation, inode cache, block cache with write-through,
  allocation and freeing, directory lookup/insert/remove, file read/write,
  truncate, cross-directory rename, unlink, fsync and stat.
- Journal every metadata transaction; write data blocks before the metadata
  that references them; preserve ordering on flush.
- On mount, replay committed journal transactions and discard incomplete ones.
- Add an in-guest fsck path and a boot self-test.
- Keep FAT32 intact; do not delete the driver.
- Gate: AFS self-test at boot, persistence write/reboot/read, host verifier
  passes, no corruption after a clean shutdown.

## Phase 3 - Migration and VFS

- Add a migration path that mounts FAT32 read-only and AFS as the new home,
  copies every file and directory, preserves names and timestamps where
  possible, verifies sizes and checksums, then switches `/home/user`.
- Keep the FAT32 volume as a read-only backup; never delete it automatically.
- Support 8.3 source names verbatim and long UTF-8 names for new files.
- Update the VFS: `FsKind::Afs`, inode tagging, `FileStat` fields for mode,
  UID, GID and inode, `fsync`, cross-directory rename and long paths.
- Update Files and Terminal so they no longer assume 8.3 names.
- Gate: migration test copies a nested tree, verifies every file with the host
  tool, reboots into AFS and keeps the FAT32 backup readable.

## Phase 4 - Permissions groundwork

- Add per-process UID/GID. The desktop and its children default to UID 1000;
  kernel threads use 0. There is no login UI yet.
- Inodes store mode, UID and GID. Enforce read/write/execute/search on open,
  read, write, execute, unlink, rename, mkdir and readdir.
- Root (UID 0) bypasses checks; document that this is a single-user model until
  0.7.1 adds accounts.
- Add the minimum syscalls for `chmod` and `chown` (next free numbers, 49/50)
  and extend `FileStat` in a versioned way.
- Migrate existing files with mode 0644/0755 and UID 1000 so the desktop keeps
  working.
- Gate: tests with two UIDs prove cross-user denial, root bypass and persisted
  mode changes; existing apps still work.

## Phase 5 - Tests, documentation and release evidence

- New tests:
  - `tools/afs_test.py`: format, mount, file/dir operations, long names,
    checksums and fsck.
  - `tools/afs_crash_test.py`: crash injection during metadata writes, reboot,
    replay and verification.
  - `tools/migration_test.py`: FAT32 tree to AFS, host verification, reboot and
    read-back.
  - `tools/perms_test.py`: UID/GID enforcement and mode changes.
- Update `persistence_test.py`, `files_probe.py`, `fs_probe.py`,
  `storage_probe.py`, `test.py`, `ui_test.py`, `app_probe.py`, `shell_probe.py`
  and `settings_test.py`.
- Documentation: `docs/architecture/AFS.md`, `STORAGE.md`, `SYSCALLS.md`,
  `docs/FEATURES.md`, `docs/PROJECT_STATUS.md`, `docs/ROADMAP.md`, `README.md`,
  `CHANGELOG.md` and `docs/releases/0.7.0/`.
- Benchmarks: sequential and random read/write, fsync, mount, migration and
  crash recovery. Record measured values only.
- Release files: `BASELINE.md`, `TEST_REPORT.md`, `PERFORMANCE.md`,
  `KNOWN_LIMITATIONS.md` and `CHANGELOG.md`.

Gate: every test green, no regressions, migration verified, docs accurate.

## Acceptance

| Area | Criterion |
|---|---|
| Build | `python tools/build.py` passes and the image verifies |
| Boot | `python tools/test.py` passes with AFS as the home |
| Format | Host tool formats and verifies an AFS image deterministically |
| Driver | Create/read/write/rename/unlink/truncate/fsync/stat/readdir work |
| Long names | UTF-8 names beyond 8.3 survive reboot and host verification |
| Crash safety | Injected crashes recover through journal replay and fsck |
| Migration | FAT32 tree copies to AFS with every file verified; backup readable |
| Permissions | UID/GID/mode enforced; root bypass; mode changes persist |
| VFS | Long paths, cross-directory rename and `FsKind::Afs` work |
| Apps | Files and Terminal work on AFS; no 8.3 assumptions |
| Honesty | No fake encryption, snapshots or "secure" claims |
| Tests | Existing and new tests green |
| Docs | Architecture, storage, syscall, feature, status, roadmap, README and release docs updated |

## Risks

| Risk | Mitigation |
|---|---|
| AFS v2 grows beyond one release | Cut snapshots and quotas first; keep checksums, journal, migration and permissions |
| Journal bugs corrupt data | Write data before metadata, checksum every record, crash-injection tests, keep FAT32 backup |
| Single-sector AHCI I/O is slow | Add a block cache and multi-sector requests through the block registry; do not bypass bounds checks |
| Migration loses data | Verify every file with the host tool, keep FAT32 read-only, never auto-delete |
| VFS ABI changes break apps | Extend `FileStat` versioned, update all consumers, run the full UI suite |
| Permissions break existing files | Default migrated files to 0644/0755 UID 1000; test the desktop before enforcing |
| Scope creep into users/login | 0.7.1 owns accounts and login; 0.7.0 only prepares UID/GID fields |

## File map

- `kernel/src/storage/afs.rs` - AFS v2 driver and journal.
- `kernel/src/storage/afs_format.rs` - on-disk structs and checksums.
- `kernel/src/fs.rs` - VFS dispatch, stat and long paths.
- `kernel/src/mount.rs` - `FsKind::Afs` and mount switching.
- `kernel/src/syscall.rs`, `shared/aster-abi/src/lib.rs` - `CHMOD`/`CHOWN`,
  `FileStat` fields and per-process UID/GID.
- `kernel/src/sched.rs` - UID/GID inheritance.
- `user/asterdesk/src/apps.rs` - Files/Terminal long-name support.
- `tools/afs_tool.py` - host formatter, inspector and verifier.
- `tools/afs_test.py`, `afs_crash_test.py`, `migration_test.py`,
  `perms_test.py` - acceptance tests.
- `docs/architecture/AFS.md` - normative format and recovery rules.
- `docs/releases/0.7.0/` - baseline, test report, performance and limits.
