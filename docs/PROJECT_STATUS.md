# Aster OS - Project Status

Version: **0.6.0 platform and devices (in progress)**
Date: 2026-11-21
Kernel: Aster 0.5.0 (higher half, x86_64)
ABI: Aster ABI 1 (syscalls 1-46 unchanged)
Desktop: Aster Desktop (ring 3, software compositor, dirty-region renderer)

## Build and test status

| Check | Result |
|---|---|
| `python tools/build.py` | PASS (bootloader, kernel, desktop, 512 MiB image verified) |
| `cargo test -p aster-motion` | PASS, 8 unit tests |
| `python tools/test.py` | PASS (host tests, motion tests, build, boot smoke, no faults) |
| Graphics acceptance (`tools/graphics_test.py`) | PASS (pixels and persisted settings) |
| UI harness (`tools/ui_test.py`) | PASS |
| App probe (`tools/app_probe.py`) | PASS (10 applications captured) |
| Pointer/dock (`tools/pointer_test.py`) | PASS |
| Window manager (`tools/wm_probe.py`) | PASS |
| Persistence (`tools/persistence_test.py`) | PASS (write, reboot, read, new settings groups) |
| Settings reliability (`tools/settings_test.py`) | PASS (corrupt/invalid/valid) |
| Service lifecycle (`tools/service_test.py`) | PASS |
| Files/filesystem/storage probes | PASS |
| Isolation (`tools/isolation_test.py`) | PASS |
| Network (`tools/net_test.py`, `network_ui_probe.py`) | PASS |
| Menu/status/about/accent/final UI probes | PASS |
| Benchmark (`tools/bench.py`) | first frame 3.61 s, frame 120 16.8 ms / 59.6 fps, 442 MiB free |

## Milestone tracker

| Phase | State | Notes |
|---|---|---|
| 0 Research/architecture | COMPLETED | Research, ADRs, design system, threat model |
| 1 Build infrastructure | COMPLETED | Reproducible image, QEMU runner, firmware fetch, tests |
| 2 First boot | COMPLETED | UEFI -> higher-half kernel -> serial |
| 3 Core kernel | PARTIAL | Memory, interrupts, scheduler, syscalls, spawn/wait, IPC, persistence; APIC/SMP and quotas missing |
| 4 Drivers/user space | PARTIAL | Serial, GOP, PS/2, PCI, polling AHCI/e1000; ACPI fixed tables, LAPIC/IOAPIC; USB, audio, GPU and MSI missing |
| 5 Graphics/desktop | PARTIAL | Dirty compositor, motion engine, window state machine, Dock 2.0, overview, switcher, tabs, effects/settings; no GPU, HiDPI, live backdrop blur or full component motion |
| 6 Applications | PARTIAL | 10 real apps, Text tabs; Store/Installer/Update/Browser absent |
| 7 Networking | PARTIAL | e1000 + IPv4 client stack; IPv6/TLS absent, polling only |
| 8 Advanced | PARTIAL | ACPI power and APIC/IOAPIC; USB, NVMe, audio, SDK missing |
| 9 Release | NOT STARTED | Optimization, hardware validation, installer media |

## Component status

| Component | State | Evidence / limitation |
|---|---|---|
| Aster Boot (UEFI) | COMPLETE for milestone | Loads kernel + modules, page tables, ExitBootServices |
| GPT/FAT32 image | COMPLETE | Deterministic builder + verifier; 8.3 names |
| PMM/VMM/heap | COMPLETE for milestone | Per-process PML4, NX/W^X, heap self-tests; no quotas/OOM policy |
| Scheduler | PARTIAL | Preemptive user threads, zombies, IPC wakeups; single CPU |
| Syscalls | PARTIAL | 46 numbers, 45 implemented; SET_THEME ENOSYS |
| Filesystem | PARTIAL | VFS + mounts, ramfs/FAT32; 8.3 names, no journal |
| PS/2 input | COMPLETE for milestone | Keyboard/mouse; no wheel/typematic |
| Graphics engine | IMPLEMENTED for milestone | Dirty regions, partial present, frame stats, quality modes, motion engine |
| Window manager | IMPLEMENTED for milestone | State machine, transitions, focus, snap, dock-targeted minimize; one window per app |
| Dock | IMPLEMENTED for milestone | Positions, size, magnification, auto-hide, indicators |
| Overview/switcher | IMPLEMENTED | Real captured previews, keyboard/mouse; snapshots not live video |
| Tabs | PARTIAL | Text editor tabs; no drag reorder/tear-out/window groups |
| Effects | PARTIAL | Real alpha/shadows/transparency; blur is wallpaper blur, not live backdrop |
| Settings | IMPLEMENTED | Graphics & Motion and prior pages, persisted through configd |
| Monitor | IMPLEMENTED | Real software-renderer telemetry, network/storage/logs |
| Networking | PARTIAL | IPv4 client stack; no inbound TCP/IPv6/TLS, polling |
| Audio/USB/GPU | NOT STARTED | Explicitly absent; no fake metrics |
| Installer/updates | NOT STARTED | Absent from UI |

## Known issues

1. Built-in apps share the desktop address space; only spawned processes are isolated.
2. Software-only graphics; no GPU, vsync, triple buffering, HiDPI or multi-monitor.
3. Blur is a cached wallpaper blur, not live composited backdrop blur.
4. Overview previews are captured at open; tabs do not support drag reorder.
5. Buttons/toggles/sliders/dialog backdrops still use immediate motion.
6. Smooth scrolling is limited to Text Page Up/Down; the PS/2 path has no wheel.
7. Dirty regions are coalesced rectangles with a 62% full-redraw fallback.
8. No central graphics resource cache or leak detector; overview/thumbs are freed on close.
9. FAT32 8.3 names/no journal; networking IPv4/polling; no KILL/ASLR/SMP.
10. First compositor frame and overview opening are expensive under QEMU TCG.

## Next tasks

| ID | Task | State |
|---|---|---|
| G1 | GPU backend with real hardware acceleration | NOT STARTED |
| G2 | Live backdrop blur and per-window surfaces | NOT STARTED |
| G3 | Drag-reorder/tear-out tabs and window groups | NOT STARTED |
| G4 | HiDPI scaling and multi-monitor placement | FOUNDATION ONLY |
| G5 | Central graphics resource cache + leak tests | NOT STARTED |
| G6 | Full component motion (buttons, toggles, dialogs) | PARTIAL |
| N1 | APIC/IOAPIC/NIC interrupts | IN PROGRESS; ACPI phase 1 landed |
| N2 | Inbound TCP + persistent network config | NOT STARTED |
| K3 | Move apps out of the desktop process | NOT STARTED |
| K6 | Per-process quotas and OOM policy | NOT STARTED |
| A1 | Audio/HDA and device ecosystem (0.6.0) | NEXT MILESTONE |

## Metrics snapshot

- Source: 46,495 tracked lines (Rust, Markdown, Python, linker scripts, TOML).
- Kernel image: ~3.03 MiB ELF (debug info included).
- Desktop image: ~4.37 MiB ELF.
- Boot: 3.61 s to first frame; kernel ready 1.74 s.
- Idle: 442 MiB free, 16.8 ms average frame at 60 fps; frames skipped when no
  dirty work; loop cadence ~20 ms while idle.
- Dirty work: ~37k px/frame during the welcome toast versus 1,024,000 px for a
  full frame.
- Overview opening: full-screen; measured ~90 ms draw in one probe, then cached.
- Network/storage metrics unchanged from 0.4.0.

## Release gate for 1.0

- user processes isolated and sandboxed;
- crash-consistent persistent filesystem;
- IPv6/TLS verified on hardware;
- real GPU backend or an honest software-only statement;
- accessibility audit and complete motion system;
- security claims backed by tests and external review;
- installer produces bootable media.
