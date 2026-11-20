# Aster OS - Project Status

Version: **0.2.0 core milestone**
Date: 2026-11-20
Kernel: Aster 0.2.0 (higher half, x86_64)
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
| Storage probe (`tools/storage_probe.py`) | PASS (Settings capacity, missing disk, corrupt volume) |
| Benchmark (`tools/bench.py`) | Recorded (see docs/benchmarks/2026-11-20.md) |
| CI | Workflow present; not yet executed on GitHub infrastructure |

## Milestone tracker

| Phase | State | Notes |
|---|---|---|
| 0 Research/architecture | COMPLETED | Research, ADRs, design system, threat model |
| 1 Build infrastructure | COMPLETED | Reproducible image, QEMU runner, firmware fetch, tests |
| 2 First boot | COMPLETED | UEFI -> higher-half kernel -> serial |
| 3 Core kernel | PARTIAL | Memory, interrupts, scheduler, syscalls done; IPC, SPAWN, APIC/SMP, demand paging missing |
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
| Scheduler | PARTIAL | Preemptive user threads, cooperative kernel threads, no SMP |
| Syscalls | PARTIAL | 31 numbers assigned, 25 implemented; SPAWN/WAIT/IPC return ENOSYS |
| Filesystem | PARTIAL | ramfs root plus a mounted FAT32 home; 8.3 names, no journal, no permissions |
| PS/2 input | COMPLETE for milestone | IRQ keyboard/mouse, ASCII + special keys, no wheel/typematic in guest |
| Framebuffer | COMPLETE | GOP 1280x800, mapped into desktop |
| Desktop shell | PARTIAL | Light menu bar with working menus, Spotlight, dock with generated icons, Control Centre, notifications, 3 desktops |
| Window manager | PARTIAL | macOS-style traffic lights and title bars, drag/resize/maximize/snap/minimize, animations; no alt-tab overlay |
| Applications | PARTIAL | 9 functional apps; no Store/Installer/Update/Browser |
| Security | PARTIAL | Ring 3, W^X, NX, pointer validation; no ASLR/SMEP/SMAP/sandbox/users |
| Networking | NOT STARTED | No Ethernet/IP/DHCP/DNS/TCP/UDP |
| Audio | NOT STARTED | No HDA/AC'97 driver; UI states this |
| Storage | PARTIAL | AHCI polling driver, GPT parsing, FAT32 home mounted at /home/user; data survives reboot |
| Installer/updates | NOT STARTED | Explicitly absent from UI |
| SDK | NOT STARTED | ABI documented; no compiler/package template yet |

## Known issues

1. System files remain in ramfs. `/home/user` is persistent only when the data disk is attached; without it the old volatile home is used.
2. Built-in apps share the desktop address space; an app crash takes down the
   shell. This is the main blocker for untrusted applications.
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
| K1 | Implement IPC channels | NOT STARTED | Two processes exchange 10k messages; handle forgery rejected |
| K2 | Implement `SPAWN`/`WAIT` | NOT STARTED | Shell starts a child, waits, gets exit code |
| K3 | Move apps to separate processes | BLOCKED on K1/K2 | Closing/crashing an app leaves the desktop alive |
| K4 | Guard pages for stacks | NOT STARTED | Overflow triggers a clean fault, not corruption |
| K5 | SMEP/SMAP/UMIP | NOT STARTED | Kernel exploit tests fail; feature logged at boot |
| D1 | PCI + ACPI discovery | PARTIAL | PCI config and AHCI work; ACPI/MADT missing |
| D2 | Persistent filesystem | COMPLETE (M1) | FAT32 home mounted; write/reboot/read passes |
| S1 | Service manager + init | BLOCKED on K1/K2 | Services restart; desktop survives a service failure |
| U1 | Damage tracking | NOT STARTED | Idle composition under 5% CPU in QEMU |
| U2 | Accessibility pass | NOT STARTED | Focus visible, reduce motion and contrast verified |
| A1 | Archive app + format | NOT STARTED | Create/extract with integrity check |

## Metrics snapshot

- Source (tracked text, excluding generated font tables): 21,677 lines across
  Rust, Markdown, Python, linker scripts, TOML, and YAML. `python tools/loc.py`
  reports the current count.
- Kernel image: ~2.3 MiB ELF (debug info included).
- Desktop image: ~2.3 MiB ELF.
- Boot: 4.69 s to first frame on QEMU TCG; kernel ready at 1.69 s.
- Idle after desktop: 398 MiB free of 510 MiB (48 MiB desktop heap included).
- Frame: ~52-60 ms idle, ~80 ms with several windows open, software rendering.

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
