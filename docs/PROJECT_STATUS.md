# Aster OS - Project Status

Version: **0.1.0 developer preview**
Date: 2026-11-20
Kernel: Aster 0.1.0 (higher half, x86_64)
ABI: Aster ABI 1
Desktop: Aster Desktop (ring 3, software compositor)

## Build and test status

| Check | Result |
|---|---|
| `python tools/build.py` | PASS (bootloader, kernel, desktop, 512 MiB image verified) |
| Host image tests (`tests/`) | PASS 4/4 |
| Boot smoke (`tools/test.py`) | PASS (BOOT-HANDOFF-OK, KERNEL-ENTRY-OK, KERNEL-READY, DESKTOP-READY, no faults) |
| UI harness (`tools/ui_test.py`) | PASS (desktop, terminal, launcher, settings, control centre screenshots) |
| App probe (`tools/app_probe.py`) | PASS (9 applications opened and captured) |
| Pointer/dock/theme test (`tools/pointer_test.py`) | PASS (dock click opens Terminal; light theme applied) |
| Window drag + virtual desktops (`tools/wm_probe.py`) | PASS (window moved; desktop switch works) |
| Benchmark (`tools/bench.py`) | Recorded (see docs/benchmarks/2026-11-20.md) |
| CI | Workflow present; not yet executed on GitHub infrastructure |

## Milestone tracker

| Phase | State | Notes |
|---|---|---|
| 0 Research/architecture | COMPLETED | Research, ADRs, design system, threat model |
| 1 Build infrastructure | COMPLETED | Reproducible image, QEMU runner, firmware fetch, tests |
| 2 First boot | COMPLETED | UEFI -> higher-half kernel -> serial |
| 3 Core kernel | PARTIAL | Memory, interrupts, scheduler, syscalls done; IPC, SPAWN, APIC/SMP, demand paging missing |
| 4 Drivers/user space | PARTIAL | Serial, GOP, PS/2, ramfs; PCI/ACPI/storage/AFS/service manager missing |
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
| Syscalls | PARTIAL | 24 calls implemented; SPAWN/WAIT/IPC/SET_THEME return ENOSYS |
| Filesystem | PARTIAL | ramfs only; no persistence, AFS not started |
| PS/2 input | COMPLETE for milestone | IRQ keyboard/mouse, ASCII + special keys, no wheel/typematic in guest |
| Framebuffer | COMPLETE | GOP 1280x800, mapped into desktop |
| Desktop shell | PARTIAL | Dock, top bar, launcher, control centre, notifications, 3 desktops |
| Window manager | PARTIAL | Drag/resize/maximize/snap/minimize, animations; no alt-tab UI overlay |
| Applications | PARTIAL | 9 functional apps; no Store/Installer/Update/Browser |
| Security | PARTIAL | Ring 3, W^X, NX, pointer validation; no ASLR/SMEP/SMAP/sandbox/users |
| Networking | NOT STARTED | No Ethernet/IP/DHCP/DNS/TCP/UDP |
| Audio | NOT STARTED | No HDA/AC'97 driver; UI states this |
| Storage | NOT STARTED | No AHCI/NVMe/VirtIO block; ramfs only |
| Installer/updates | NOT STARTED | Explicitly absent from UI |
| SDK | NOT STARTED | ABI documented; no compiler/package template yet |

## Known issues

1. Ramfs is volatile. Settings and files disappear on shutdown.
2. Built-in apps share the desktop address space; an app crash takes down the
   shell. This is the main blocker for untrusted applications.
3. Desktop startup maps its 48 MiB heap eagerly (~2.5 s under TCG).
4. Software composition runs at ~18 fps at 1280x800 under QEMU TCG; no damage
   tracking.
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
11. Settings persistence is per-session only (ramfs).
12. The Calendar and top bar depend on QEMU's `-rtc base=localtime` for host
    local time.

## Next tasks (ordered)

| ID | Task | State | Acceptance |
|---|---|---|---|
| K1 | Implement IPC channels | NOT STARTED | Two processes exchange 10k messages; handle forgery rejected |
| K2 | Implement `SPAWN`/`WAIT` | NOT STARTED | Shell starts a child, waits, gets exit code |
| K3 | Move apps to separate processes | BLOCKED on K1/K2 | Closing/crashing an app leaves the desktop alive |
| K4 | Guard pages for stacks | NOT STARTED | Overflow triggers a clean fault, not corruption |
| K5 | SMEP/SMAP/UMIP | NOT STARTED | Kernel exploit tests fail; feature logged at boot |
| D1 | PCI + ACPI discovery | NOT STARTED | Enumerates q35 devices and MADT |
| D2 | VirtIO block + AFS v1 | BLOCKED on D1 | Mount, read, checksum-verify a test image |
| S1 | Service manager + init | BLOCKED on K1/K2 | Services restart; desktop survives a service failure |
| U1 | Damage tracking | NOT STARTED | Idle composition under 5% CPU in QEMU |
| U2 | Accessibility pass | NOT STARTED | Focus visible, reduce motion and contrast verified |
| A1 | Archive app + format | NOT STARTED | Create/extract with integrity check |

## Metrics snapshot

- Source (tracked text, excluding generated font tables): 21,480 lines across
  Rust, Markdown, Python, linker scripts, TOML, and YAML. `python tools/loc.py`
  reports the current count.
- Kernel image: ~2.3 MiB ELF (debug info included).
- Desktop image: ~2.3 MiB ELF.
- Boot: 4.24 s to first frame on QEMU TCG; kernel ready at 1.65 s.
- Idle after desktop: 407 MiB free of 510 MiB (48 MiB desktop heap included).
- Frame: ~54 ms (~18 fps) at 1280x800, software rendering.

## Naming note

The workspace, title, and all code use **Aster OS** (codename Aster). The
original long-form brief used "NOVA OS" in its body; that name is treated as a
legacy placeholder. No architectural meaning is attached to either name.

## Release gate for 1.0

Aster OS will not be called a usable desktop OS until, at minimum:

- user processes can be spawned, isolated, and sandboxed;
- files persist across reboots on the native filesystem;
- networking and updates work and are verified;
- the desktop meets its accessibility and performance targets on real hardware;
- security claims are backed by tests and an external review;
- the installer produces a bootable system from release media.
