# Aster OS - Project Status

Version: **0.3.0 storage and services**
Date: 2026-11-20
Kernel: Aster 0.3.0 (higher half, x86_64)
ABI: Aster ABI 1
Desktop: Aster Desktop (ring 3, light macOS-inspired shell, software compositor)

## Build and test status

| Check | Result |
|---|---|
| `python tools/build.py` | PASS (bootloader, kernel, desktop, 512 MiB image verified) |
| Host image tests (`tests/`) | PASS 4/4 |
| Boot smoke (`tools/test.py`) | PASS (BOOT-HANDOFF-OK, KERNEL-ENTRY-OK, KERNEL-READY, DESKTOP-READY, no faults) |
| UI harness (`tools/ui_test.py`) | PASS (desktop, Terminal help/ls/cat, Settings, Files, Spotlight screenshots) |
| App probe (`tools/app_probe.py`) | PASS (9 applications captured; Wallpapers may need a longer first-open wait) |
| Pointer/dock test (`tools/pointer_test.py`) | PASS (dock click, notification centre, Control Centre, brightness) |
| Window drag + virtual desktops (`tools/wm_probe.py`) | PASS (window moved; desktop switch works) |
| Menu/About/Centre probes (`shell_probe.py`, `about_probe.py`, `final_ui_probe.py`) | PASS (menus, About logo, Control Centre, wallpaper apply) |
| Persistence (`tools/persistence_test.py`) | PASS (write, reboot, read; host FAT32 verifier) |
| Settings reliability (`tools/settings_test.py`) | PASS (corrupt, invalid values, valid round-trip) |
| Service lifecycle (`tools/service_test.py`) | PASS (GET/SET, exit, restart, save after restart) |
| Files operations (`tools/files_probe.py`) | PASS (create, rename, copy, paste, delete, search, info) |
| Filesystem probe (`tools/fs_probe.py`) | PASS (64 KiB write/read, rename, delete, sync) |
| Status pages (`tools/status_probe.py`) | PASS (Monitor logs, Settings storage) |
| Storage probe (`tools/storage_probe.py`) | PASS (Settings capacity, missing disk, corrupt volume) |
| Benchmark (`tools/bench.py`) | Recorded (see docs/benchmarks/2026-11-20.md) |
| CI | Workflow present; not yet executed on GitHub infrastructure |

## Milestone tracker

| Phase | State | Notes |
|---|---|---|
| 0 Research/architecture | COMPLETED | Research, ADRs, design system, threat model |
| 1 Build infrastructure | COMPLETED | Reproducible image, QEMU runner, firmware fetch, tests |
| 2 First boot | COMPLETED | UEFI -> higher-half kernel -> serial |
| 3 Core kernel | PARTIAL | Memory, interrupts, scheduler, syscalls, spawn/wait, IPC and persistence done; service manager, APIC/SMP and demand paging missing |
| 4 Drivers/user space | PARTIAL | Serial, GOP, PS/2, PCI config, AHCI, GPT/FAT32; ACPI/APIC/service manager missing |
| 5 Graphics/desktop | PARTIAL | Compositor, WM, shell, settings; damage tracking, HiDPI, accessibility, GPU missing |
| 6 Applications | PARTIAL | 9 real apps; Store/Installer/Update/Browser deliberately absent |
| 7 Networking/security | NOT STARTED | No network, users, sandbox, packages, updates |
| 8 Advanced | NOT STARTED | USB, audio, NVMe, power, SDK |
| 9 Release | NOT STARTED | Optimization, hardware validation, installer media |

## Component status

| Component | State | Evidence / limitation |
|---|---|---|
| Aster Boot (UEFI) | COMPLETE for milestone | Loads kernel + module, builds page tables, ExitBootServices |
| GPT/FAT32 image | COMPLETE | Deterministic builder + verifier; 8.3 names only |
| GDT/TSS/IDT | COMPLETE | 256 vectors, IST double fault, DPL3 syscall gate |
| PMM | COMPLETE (32 GiB cap) | Bitmap, zeroing, reservations, accounting |
| VMM | COMPLETE for milestone | 4-level, U/S on all levels, per-process PML4, NX/W^X |
| Kernel heap | COMPLETE | first-fit, coalescing, GlobalAlloc, self-test |
| Scheduler | PARTIAL | Preemptive user threads, zombies, event wake-up for WAIT/IPC, cooperative kernel threads, no SMP |
| Syscalls | PARTIAL | 36 numbers assigned, 35 implemented; SET_THEME returns ENOSYS (light appearance is fixed) |
| Filesystem | PARTIAL | VFS + mount table, ramfs root and FAT32 home; rename/sync; 8.3 names, no journal, no permissions |
| PS/2 input | COMPLETE for milestone | IRQ keyboard/mouse, ASCII + special keys, no wheel/typematic in guest |
| Framebuffer | COMPLETE | GOP 1280x800, mapped into desktop |
| Desktop shell | PARTIAL | Light menu bar, Spotlight, dock, Control Centre, notifications, Files operations, Monitor storage/logs |
| Window manager | PARTIAL | macOS-style traffic lights and title bars, drag/resize/maximize/snap/minimize, animations; no alt-tab overlay |
| Applications | PARTIAL | 9 functional apps; no Store/Installer/Update/Browser |
| Security | PARTIAL | Ring 3, W^X, NX, pointer validation; no ASLR/SMEP/SMAP/sandbox/users |
| Settings | IMPLEMENTED | Versioned, validated, backup/restore, configd service, direct fallback |
| Services | PARTIAL | configd + supervisor restart/timeout/fallback; no generic manager or KILL |
| Logging | PARTIAL | Levels + timestamps + subsystem prefixes; LOG_READ + Monitor view; volatile ring only |
| Networking | NOT STARTED | No Ethernet/IP/DHCP/DNS/TCP/UDP |
| Audio | NOT STARTED | No HDA/AC'97 driver; UI states this |
| Storage | PARTIAL | Block registry (bounds/flush/counters), mount table, GPT/FAT32, STORAGE_DETAILS; single-sector DMA |
| Installer/updates | NOT STARTED | Explicitly absent from UI |
| SDK | NOT STARTED | ABI documented; no compiler/package template yet |

## Known issues

1. System files remain in ramfs. `/home/user` is persistent only when the data disk is attached; without it the old volatile home is used. The volume uses 8.3 names and has no journal.
2. Built-in apps share the desktop address space; an app crash takes down the
   shell. Spawned child processes are isolated, but the apps have not moved yet.
3. Desktop startup maps its 48 MiB heap eagerly and prepares wallpapers/icons (~2.5 s under TCG).
4. Software composition runs at roughly 50-80 ms per frame at 1280x800 under
   QEMU TCG depending on open windows; no damage tracking.
5. User binaries are linked at 4 MiB because the current toolchain uses the
   small code model. Raising user space requires a large-code-model user build.
6. No keyboard typematic repeat or mouse wheel events in the guest yet.
7. The ESP uses 8.3 file names; the loader does not parse long file names.
8. No guard pages around kernel/user stacks; stack overflow would corrupt
   adjacent memory.
9. No window snapping/resize automated test yet (drag and desktop switching are
   covered).
10. Toast notifications overlap windows in the top-right until they expire;
    they pause/dismiss only by timeout or the notification panel.
11. The persistent volume uses 8.3 uppercase names and has no journal or permissions; a power loss can truncate a file.
12. IPC receive has a timeout; send has none. No bulk shared-memory path or arbitrary handle transfer; queues hold 8 x 256 bytes.
13. There is one managed service (configd) and no KILL; a hung service cannot be forcibly restarted.
14. Files copy is recursive but depth-limited and has no progress UI; search is per-directory.
12. The Edit menu's Undo/Cut/Copy/Paste entries are disabled because selection
    and clipboard services are not implemented.
13. Opening Wallpapers for the first time regenerates four previews and may
    take a few seconds under emulation.
14. The Calendar and top bar depend on QEMU's `-rtc base=localtime` for host
    local time.

## Next tasks (ordered)

| ID | Task | State | Acceptance |
|---|---|---|---|
| K0 | Persistent home volume | COMPLETE (M1) | Write, reboot, read verified by host parser; corrupt/missing disk handled |
| M4 | Service manager + first service | NEXT | A service starts over a bootstrap channel and survives/restarts independently |
| K1 | Implement IPC channels | CORE COMPLETE (M3) | `ipc`/`ipchup`: ping-pong, forged handle -3, hangup -14 |
| K2 | Implement `SPAWN`/`WAIT` | COMPLETE (M2) | Isolation command spawns two children, reports 42 and -11, desktop survives |
| K3 | Move apps to separate processes | NEXT (K1/K2 done) | Closing/crashing an app leaves the desktop alive |
| K4 | Guard pages for stacks | NOT STARTED | Overflow triggers a clean fault, not corruption |
| K6 | Per-process memory quotas | NOT STARTED | A child cannot exhaust physical memory |
| K5 | SMEP/SMAP/UMIP | NOT STARTED | Kernel exploit tests fail; feature logged at boot |
| D1 | PCI + ACPI discovery | PARTIAL | PCI config and AHCI work; ACPI/MADT missing |
| D2 | Persistent filesystem | COMPLETE (M1) | FAT32 home mounted; write/reboot/read passes |
| S1 | Service manager + init | PARTIAL (configd supervisor) | configd restarts; desktop survives failure; generic manager/KILL remain |
| U1 | Damage tracking | NOT STARTED | Idle composition under 5% CPU in QEMU |
| U2 | Accessibility pass | NOT STARTED | Focus visible, reduce motion and contrast verified |
| A1 | Archive app + format | NOT STARTED | Create/extract with integrity check |

## Metrics snapshot

- Source (tracked text, excluding generated font tables): 21,677 lines across
  Rust, Markdown, Python, linker scripts, TOML, and YAML. `python tools/loc.py`
  reports the current count.
- Kernel image: ~2.3 MiB ELF (debug info included).
- Desktop image: ~2.3 MiB ELF.
- Boot: 4.90 s to first frame on QEMU TCG; kernel ready at 1.84 s.
- Idle after desktop: 396 MiB free of 510 MiB (48 MiB desktop heap included).
- Frame: ~47 ms idle; brightness below 100% adds a full-surface blend.
- Storage: 64 KiB FAT32 write ~19 ms, read ~17 ms (single-sector AHCI).

## Naming note

The workspace, title, and all code use **Aster OS** (codename Aster). The
original long-form brief used "NOVA OS" in its body; that name is treated as a
legacy placeholder. No architectural meaning is attached to either name.

## Release gate for 1.0

Aster OS will not be called a usable desktop OS until, at minimum:

- user processes can be spawned, isolated, and sandboxed;
- files persist across reboots with crash consistency;
- networking and updates work and are verified;
- the desktop meets its accessibility and performance targets on real hardware;
- security claims are backed by tests and an external review;
- the installer produces a bootable system from release media.
