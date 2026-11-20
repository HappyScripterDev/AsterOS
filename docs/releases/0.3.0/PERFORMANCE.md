# Aster OS 0.3.0 - Performance

Date: 2026-11-20
Machine: QEMU TCG, one vCPU, 512 MiB, 1280x800, std VGA, boot disk + data disk.
Numbers are from `tools/bench.py` with a freshly built volume and from the
`fstest` command in `tools/fs_probe.py`.

## 1. Boot

| Stage | 0.2.0 | 0.3.0 |
|---|---|---|
| OVMF to Aster Boot | 1.455 s | 1.594 s |
| Kernel loaded, ExitBootServices | 1.647 s | 1.792 s |
| Kernel initialised | 1.672 s | 1.840 s |
| Desktop initialised | 4.643 s | 4.790 s |
| First compositor frame | 4.751 s | 4.900 s |

The increase is dominated by loading five boot modules (desktop, three probes,
configd) instead of four, plus the configd start and PING handshake. Kernel
storage initialisation itself remains inside the ~1.8 s kernel window.

## 2. Memory

| Metric | Value |
|---|---|
| RAM | 510 MiB |
| Free after kernel init | 444 MiB |
| Kernel heap | 2048 KiB mapped, 71 KiB used |
| Free after desktop heap | 396 MiB |
| Desktop heap | 48 MiB |
| configd image | ~739 KiB (debug info included) |

## 3. Desktop

| Metric | Value |
|---|---|
| Idle frame 120 average | 46.8 ms (draw ~44-54 ms, input 0-1 ms) |
| Startup frames | 60-90 ms while wallpapers/shadows are built |
| Brightness below 100% | ~77-79 ms (full-surface dimming overlay) |

The Files list now draws truncated text without allocating a `String` per
call. Previously `ui::truncate` allocated a `String` and one per character; on
the bump-allocated desktop heap that was an unbounded per-frame leak. The new
`draw_text_truncated` uses glyph advances and draws in place.

## 4. Filesystem

`fstest`, 64 KiB file on FAT32 over single-sector AHCI DMA:

| Operation | Time | Throughput |
|---|---|---|
| Write 64 KiB | 19 ms | ~3.4 MiB/s |
| Read 64 KiB | 17 ms | ~3.7 MiB/s |
| Rename | < 1 ms (one directory-entry write) | - |
| Delete | < 1 ms plus chain free | - |

The write path performs 128 sector writes plus FAT and directory updates. A
multi-sector DMA transfer and a FAT sector cache are the obvious next
optimisations; correctness was prioritised in 0.3.0.

## 5. Services

- configd start + PING + GET completes before the desktop is declared ready.
- A settings SET (`accent`) is a 4-byte message plus a ~100-byte reply and a
  FAT write; the service test completes within the normal 1 s UI wait.
- IPC receive timeout for service calls is 700-900 ms; the desktop falls back
  to direct file I/O if the service does not answer.
- Supervisor check is one `PROC_LIST` syscall every 2 s.

No optimisation was claimed without a measurement. The biggest remaining
performance costs are full-frame composition, the eager 48 MiB desktop heap and
single-sector disk I/O.
