# Testing Aster OS

Aster OS is tested at four levels: host tooling, kernel self-tests, boot-level
integration tests, and scripted UI interaction. Tests that have not run are not
presented as passing.

## 1. Commands

```bash
# Host-side image/GPT/FAT tests (no QEMU)
python -m unittest discover -s tests -v

# Build, then boot headlessly and assert serial milestones
python tools/test.py

# Also run the scripted launcher/dock/keyboard/control-centre scenario
python tools/test.py --ui

# Additional interaction harnesses
python tools/ui_test.py
python tools/app_probe.py
python tools/pointer_test.py
python tools/wm_probe.py

# Boot/frame benchmark
python tools/bench.py
```

All scripts write logs and screenshots under `build/`. `tools/test.py` exits
non-zero on a missing marker, a kernel panic, a user fault, or a failed host
test.

## 2. What each layer covers

### Host tests (`tests/`)

`test_image.py` builds a 300 MiB GPT/FAT32 image and asserts:

- GPT signature and header CRC,
- FAT32 directory traversal and file payload round-trip,
- corruption in the GPT header is detected,
- names that cannot be represented in 8.3 are rejected.

These run without QEMU and are fast (<1 s).

### Kernel self-tests (boot-time, serial)

- `heap::self_test`: 256 variable-size allocations with content checks, free,
  then a 512 KiB allocation; asserts no leak in the allocated counter.
- `mm::selftest::frame_test`: allocate a zeroed frame, write patterns through
  the direct map, map it at a kernel VA, verify translation and reads, unmap,
  free.
- `mm::selftest::user_space_test`: create a user PML4, allocate 8 KiB, copy
  data in both directions, destroy the address space, assert the page-table
  pages were returned to the PMM.
- `fs::self_test`: mkdir, write, read, list, unlink, and verify removal.

A failure halts the kernel with a diagnostic; it cannot silently continue.

### Boot integration (`tools/test.py`)

Boots the real GPT image under OVMF and asserts these serial markers in order:

```
BOOT-LOADER-START
BOOT-HANDOFF-OK
KERNEL-ENTRY-OK
KERNEL-READY
DESKTOP-READY
```

It also captures a screenshot and fails on `KERNEL PANIC`, `user fault`, or
`system halted`.

### UI interaction (`tools/ui_test.py`)

Drives QEMU via QMP:

1. screenshot the desktop,
2. open Terminal through the launcher (`Ctrl+Space`, type `term`, Enter),
3. type `help` and assert the screenshot is non-trivial (manual review),
4. open the launcher and search for Settings,
5. open Settings and the control centre.

`app_probe.py` opens every built-in app in turn through the launcher and
captures a screenshot. `pointer_test.py` verifies relative pointer motion, the
dock, the control centre, and the dark/light theme toggle. `wm_probe.py`
verifies titlebar dragging and virtual desktop switching.

These probes assert that the system stays alive and produces frames; they do
not yet do pixel-diff assertions. Screenshots are reviewed and attached to
releases.

### Benchmarks (`tools/bench.py`)

Measures wall-clock time to each serial marker from QEMU launch, plus the
desktop's own frame-time log. Results are recorded in
`docs/benchmarks/2026-11-20.md`.

## 3. Current test matrix

| Area | Automated | Manual | Notes |
|---|---|---|---|
| Boot chain (UEFI -> kernel -> desktop) | Yes | - | `tools/test.py` |
| PMM/VMM/heap invariants | Yes | - | boot self-tests |
| Ramfs semantics | Yes | - | boot self-test |
| Syscall validation | Partial | - | exercised by apps; no adversarial fuzzing yet |
| PS/2 keyboard/mouse | Yes | Yes | QMP probes + interactive |
| Window manager | Partial | Yes | drag/desktop probe; resize/snap manual |
| Launcher/control centre/theme | Yes | Yes | UI probes |
| Terminal commands | Manual | Yes | `help`, `ls`, `cat`, `write`, `theme` |
| Files/Text/Notes persistence | Manual | Yes | ramfs only; lost on shutdown |
| Networking/audio/storage | Not applicable | - | not implemented |
| SMP/APIC | Not applicable | - | not implemented |
| Security isolation | Partial | - | ring-3 faults are handled; no exploit tests |

## 4. CI

`.github/workflows/ci.yml` runs host tests, a build, and the boot smoke test on
Ubuntu with QEMU TCG. The UI probes are run locally because they depend on VNC
and longer timeouts; they can be enabled in CI later.

## 5. Missing coverage (tracked)

- No fuzzing of syscall arguments or ELF parsing.
- No fault-injection tests (out-of-memory, disk failure, corrupted ramfs).
- No SMP or interrupt-storm stress tests.
- No real-hardware test farm yet.
- No pixel-diff regression baselines for the desktop.
- No automated test for window resizing, snapping, or the notification panel.
- No test for the planned IPC or `SPAWN`.

Each gap is listed in `docs/PROJECT_STATUS.md` with the milestone that should
close it.
