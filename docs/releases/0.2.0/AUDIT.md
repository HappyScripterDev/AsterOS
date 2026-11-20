# Aster OS 0.2.0 - Release Audit

Date: 2026-11-20
Baseline: `v0.1.0-baseline` (`fc8caf3`, branch `milestone/aster-0.2-core`)
Detailed baseline audit: [../../roadmap/ASTER_0.2_AUDIT.md](../../roadmap/ASTER_0.2_AUDIT.md)
Roadmap: [../../roadmap/ASTER_0.2_ROADMAP.md](../../roadmap/ASTER_0.2_ROADMAP.md)

## 1. Result

Aster OS 0.2.0 reaches the **core milestone**: isolated processes, validated
syscalls, native IPC and persistent storage are implemented and verified. The
desktop, all nine applications and the original boot path still work.

Not delivered: a service manager, per-process memory quotas, IPC timeouts and
bulk shared memory, SMEP/SMAP/UMIP, ASLR, guard pages and app extraction. They
are listed as follow-ups, not implied by the release.

## 2. Verified baseline (0.1.0)

- Build, host image tests, boot smoke and UI probes passed before any change.
- Boot to first frame: 5.30 s wall, 2.25 s in-guest.
- Idle frame time: 52-60 ms at 1280x800 under QEMU TCG.
- No process creation, no IPC, no persistent storage.
- Only page faults from ring 3 were contained; other user exceptions halted the
  machine.
- The desktop was one ring-3 process with nine in-process applications.

## 3. M1: persistent home volume

| Component | Evidence |
|---|---|
| PCI config + AHCI | `ahci: 2 disk(s) ready`, IDENTIFY model and sector counts |
| GPT parsing | Data partition at LBA 2048, header CRC verified |
| FAT32 mount | `storage: FAT32 'ASTERDATA' mounted, 130552 clusters of 4096 bytes` |
| Create/read/write/mkdir/unlink | Boot self-test creates, reads, truncates and removes files and a directory |
| Reboot persistence | `tools/persistence_test.py`: guest writes, host verifier parses, reboot, guest reads back |
| Settings persistence | Accent change survives the reboot; host verifies `SETTINGS.CNF` |
| Capacity reporting | Settings > Storage shows 509.9 MiB total/free and mount state |
| Missing/corrupt disk | Boot reaches the desktop; mount rejected; no fault |

## 4. M2: process isolation

- `SPAWN(path)` loads an ELF from the VFS or a boot module, validates it
  (bounds, W^X, entry in RX, no overlaps), creates a private PML4, maps a stack
  and registers the child with its parent.
- `WAIT(pid, &status)` blocks until a child is a zombie, destroys its address
  space and returns the exact status. Orphans are reaped.
- Every user exception is contained: a child that writes to kernel memory exits
  with status -11 while the desktop survives.
- Boot self-test maps the same VA in two address spaces to different frames,
  verifies private writes, rejects kernel addresses as user buffers and checks
  frame reclamation.
- Evidence: `tools/isolation_test.py` prints status 42 and -11, then the
  desktop still accepts `uname`.

## 5. M3: IPC

- `CHANNEL_CREATE/SEND/RECV/CLOSE`, per-process handles, 8 x 256 B queues,
  blocking send/receive and close/hangup.
- `SPAWN(path, handle)` duplicates one endpoint into the child at handle 3.
- Evidence: `ipc` returns `pong` and child status 0; `ipchup` reports `-3` for a
  forged handle and `-14` after the peer exits; `systest` reports `-5` for four
  invalid syscall arguments.
- No timeouts, shared memory or arbitrary handle transfer yet.

## 6. Measured performance (clean volume, QEMU TCG)

| Metric | Value |
|---|---|
| OVMF to Aster Boot | 1.455 s |
| Kernel loaded, ExitBootServices | 1.647 s |
| Kernel initialised | 1.672 s |
| Desktop initialised | 4.643 s |
| First compositor frame | 4.751 s |
| Steady frame (frame 120) | 47.2 ms average |
| Free RAM after desktop heap | 397 MiB of 510 MiB |
| Kernel heap | 2048 KiB |

A separate run with the interactive volume's brightness at 0.97 measured
77-79 ms per frame. The dimming overlay blends the full 1280x800 surface every
frame; it is a real cost of that setting, not a regression. Benchmarks now
create a clean volume with brightness 1.00.

## 7. Definition of done

| Requirement | State |
|---|---|
| Boots reliably; desktop and apps work | PASS |
| Isolated address spaces per process | PASS |
| Memory permissions / user-kernel separation | PASS (NX, W^X, CR0.WP, user/write checks) |
| Process termination releases resources | PASS (zombies, owned-frame teardown; no quotas) |
| IPC between isolated processes | PASS |
| Invalid IPC handled safely | PASS (EBADF, EPIPE) |
| Persistent storage; data survives reboot | PASS |
| Syscall argument validation | PASS (probe + review; SHUTDOWN still unprivileged) |
| Regression and new core tests | PASS |
| Performance and limitations documented | PASS |
| Service architecture started | NOT DONE (M4) |

## 8. Remaining work for the next milestone

1. Service manager and the first extracted service (storage or process service).
2. Per-process page quotas and a global memory reserve.
3. IPC timeouts, shared memory and handle transfer.
4. Guard pages, ASLR, SMEP/SMAP/UMIP.
5. Moving built-in apps to separate processes with window surfaces.
6. 8.3-name removal (VFAT long names) and journaling for the persistent volume.
7. Damage tracking and a faster brightness path.
