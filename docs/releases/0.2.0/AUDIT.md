# Aster OS 0.2.0 - Release Audit

Date: 2026-11-20
Baseline: `v0.1.0-baseline` (`fc8caf3`, branch `milestone/aster-0.2-core`)
Detailed baseline audit: [../../roadmap/ASTER_0.2_AUDIT.md](../../roadmap/ASTER_0.2_AUDIT.md)
Roadmap: [../../roadmap/ASTER_0.2_ROADMAP.md](../../roadmap/ASTER_0.2_ROADMAP.md)

This page is the release-facing summary. The detailed audit records the
pre-change state; this page records what changed in the first 0.2 milestone and
what remains.

## 1. Verified baseline (0.1.0)

- Build, host image tests, boot smoke and UI probes passed before any change.
- Boot to first frame: 5.30 s wall, 2.25 s in-guest.
- Idle frame time: 52-60 ms at 1280x800 under QEMU TCG.
- No process creation, no IPC, no persistent storage.
- Only page faults from ring 3 were contained; other user exceptions halted the
  machine.
- The desktop was one ring-3 process with nine in-process applications.

## 2. M1 result: persistent home volume

Shipped:

| Component | Evidence |
|---|---|
| PCI config + AHCI driver | Boot log: `ahci: 2 disk(s) ready`, IDENTIFY model and sector counts |
| GPT parsing | Data partition found at LBA 2048 with a verified header CRC |
| FAT32 mount | `storage: FAT32 'ASTERDATA' mounted, 130552 clusters of 4096 bytes` |
| Read/write/create/mkdir/unlink | Boot self-test creates, reads, truncates and removes a file and a directory |
| Persistent home | `tools/persistence_test.py`: guest writes, host verifier parses, reboot, guest reads back |
| Settings persistence | Accent change survives the reboot; host verifier checks `SETTINGS.CNF` |
| Real capacity reporting | Settings > Storage shows 509.9 MiB total/free and mount state |
| Missing disk | Boot reaches the desktop; log: `storage: no persistent volume mounted` |
| Corrupt volume | Zeroed FAT boot sector is rejected; desktop starts; no fault |
| Kernel hardening | Full user-exception containment, PMM frame bounds, mapping rollback, heap coalescing/1 MiB, user-copy permissions, `PTE_OWNED`, NX/WP, ELF W^X |

Measured after M1:

| Metric | Value |
|---|---|
| Firmware to first frame | 4.69 s |
| Kernel ready | 1.69 s |
| Desktop init (in guest) | 2.15 s |
| Free RAM after desktop | 398 MiB of 510 MiB |
| Volume | 512 MiB disk, 509.9 MiB FAT32, 4 KiB clusters |

## 3. Remaining gaps for 0.2.0

1. **Process isolation**: `SPAWN`, `WAIT`, zombie/exit-status handling and
   per-process quotas are still absent. The desktop remains PID 1 and apps
   share its address space.
2. **IPC**: channel syscalls still return `ENOSYS`; there are no handles, wait
   queues or blocking receive.
3. **System services**: nothing has been extracted; the kernel owns the
   filesystem.
4. **Storage limits**: 8.3 names, no journal, no permissions, single volume,
   single-sector DMA.
5. **Security gaps**: no SMEP/SMAP/UMIP, no capabilities, no ASLR, no kernel
   stack guards.
6. **Performance**: full-frame composition, eager 48 MiB desktop heap, no
   damage tracking.

## 4. Honest status

Aster OS 0.2.0 is **not complete**. The first milestone is complete and
verified. Process isolation and IPC are the next milestone and are prerequisites
for service extraction.
