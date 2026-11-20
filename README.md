# Aster OS

**A new generation of personal computing.**

Aster OS is an independent, from-scratch desktop operating system for x86_64.
It is **not** a Linux distribution, **not** a modified Windows or macOS, and
**not** a desktop mockup. It has its own UEFI bootloader, higher-half kernel,
memory manager, scheduler, system-call ABI, filesystem, input stack, and a
ring-3 desktop environment with a light, macOS-inspired software compositor, isolated child processes, a block/mount storage stack, a persistent FAT32 home volume and a user-space settings service.

![Aster Desktop](docs/screenshots/desktop.png)

## Status: 0.3.0 storage and services

This repository contains a working, bootable system on QEMU (UEFI + GPT/FAT32
disk image). The current milestone is a **single-CPU developer preview**:

| Area | What actually works |
|---|---|
| Boot | Aster Boot (own UEFI loader), GPT + FAT32 ESP, ELF kernel loading, custom page tables, `ExitBootServices`, higher-half handoff |
| Kernel | GDT/TSS, IDT, 256 vectors, exceptions, PIC/PIT at 1 kHz, 4-level paging, bitmap frame allocator, kernel heap (`alloc`), preemptive scheduler, ring-3 processes, syscalls via `int 0x80` |
| Memory | 4 KiB frames, 32 GiB addressable bitmap, per-process PML4, NX/W^X user mappings, heap self-tests, user copy validation |
| Drivers | COM1 serial, UEFI GOP framebuffer, PS/2 keyboard + mouse (IRQ driven), PIT, PCI config, AHCI (q35) |
| Filesystem | In-memory ramfs with paths, directories, files, file descriptors, `readdir`, `stat`, `unlink`, `mkdir` |
| Desktop | Aster Desktop in ring 3: light menu bar, Spotlight launcher, dock with generated app icons and magnification, macOS-style window manager (traffic lights, drag, resize, maximize, snap, animations), Control Centre, notifications, virtual desktops |
| Storage | Block registry with capacity/bounds/flush/counters, mount table, GPT + FAT32 at `/home/user`; v1 settings with backup; Settings reports device, filesystem, capacity, used/free and I/O |
| Processes | Private PML4 per process; `SPAWN`/`WAIT`; zombie reaping; every ring-3 exception contained; isolation probe with exit status 42 and a deliberate fault returning -11 |
| Services | `configd` (ring 3) owns validated settings over IPC; desktop supervisor restarts it; direct-I/O fallback; timed IPC receive |
| Files | Real create, rename, delete, copy, move, info, sort, search, forward/back and breadcrumbs on the mounted filesystem |
| Monitor | CPU/memory/process cards, storage usage, real I/O counters and a kernel-log view |
| IPC | Kernel channels with per-process handles, 256-byte messages, blocking send/receive, close/hangup; ping-pong verified between isolated processes |
| Apps | Terminal (real shell + ramfs), Files, Settings, Calculator, Monitor, Text, Notes, Calendar, Wallpapers |
| Tests | Image verifier, kernel self-tests, ramfs self-test, QEMU boot smoke test, scripted UI/pointer tests |

Everything in the table is exercised by booting the system and driving it, not
by static screenshots. Known gaps are listed in
[docs/FEATURES.md](docs/FEATURES.md) and [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md).
There is no networking, audio, USB, storage driver, AFS persistence, SMP/APIC,
GPU acceleration, application sandboxing, package manager, installer, updater,
or browser yet. Store/Installer/Update/Browser are deliberately **not** present
in the UI.

## Quick start

Requirements: Rust (stable), QEMU 8+, OVMF/UEFI firmware, Python 3.10+.
The build uses `rust-lld`; Clang and NASM are not required.

```bash
rustup target add x86_64-unknown-none x86_64-unknown-uefi
python tools/fetch_firmware.py        # downloads the OVMF test firmware (BSD)
python tools/build.py                 # bootloader + kernel + desktop + disk image
python tools/run.py                   # boot in QEMU (interactive window + serial)
```

Headless smoke test:

```bash
python tools/run.py --headless --timeout 12 --screenshot build/screen.ppm
python tools/test.py                  # build + host image tests + boot assertions
```

Full instructions, supported hosts, and troubleshooting:
[docs/development/BUILDING.md](docs/development/BUILDING.md).

## Desktop

The desktop runs as PID 1 in ring 3 (`/DESK` loaded by the bootloader). The
kernel only exposes a framebuffer mapping, input events, time, memory, and
filesystem syscalls; it contains no UI code.

- Menu bar: Aster brand tile, application name, working File/Edit/View/Window/Help
  menus, Spotlight, Control Centre, Wi-Fi status, clock and notification centre.
- Dock: generated squircle app icons, magnification, running indicators,
  tooltips, click to launch or focus.
- Spotlight: `Ctrl+Space`, centred translucent search panel, keyboard
  navigation, filtered results; the panel resizes to the result count.
- Windows: macOS-style traffic lights and centred titles, drag, eight-way
  resize, minimize to the dock, maximize, snap to halves, rounded corners,
  cached soft shadows and light title bars.
- Control Centre: light-only appearance, real display brightness, reduce
  motion, accent colour, and honest "No adapter" rows for Wi-Fi, Bluetooth and
  audio. There is no dark mode in this build.
- Shortcuts: `Ctrl+Space` Spotlight, `Ctrl+W` close, `Ctrl+M` minimize,
  `Ctrl+Tab` cycle, `Ctrl+Left/Right` virtual desktops.

### Design

![Files](docs/screenshots/files.png)

![Persistence after reboot](docs/screenshots/persistence.png)

Storage is layered: AHCI -> block registry -> mount table -> FAT32/ramfs, with
capacity checks, flush and I/O counters. The home directory is a real FAT32
volume on a second QEMU disk. Files,
notes and preferences survive a reboot; `tools/persistence_test.py` writes in
the guest, verifies the volume with an independent host parser, reboots, and
reads the data back. The boot image is never written by the kernel. The volume
uses 8.3 names and has no journal yet, and Settings states both limits.

Settings are versioned (`version=1`), validated per key and written through a
temporary file with a backup. `configd` owns them in the normal path; the
desktop supervisor restarts it on failure and falls back to direct I/O so the
session is never locked out. See [0.3.0 release docs](docs/releases/0.3.0/).

The desktop is light-only by product decision and follows macOS interaction
patterns without copying Apple code or artwork: a translucent menu bar, a
Spotlight-style launcher, traffic-light window controls, a bottom dock and
soft, layered surfaces. The supplied Aster logo (`assets/Logo/Logo.png`) is
used in the menu bar, notifications and the About page. The nine app icons are
original artwork generated by `tools/icongen.py`; the four wallpapers are
procedural (Sonoma, Sequoia, Ventura, Monterey). All icons and wallpapers are
drawn or generated by this project.

Design tokens and interaction rules: [docs/design/](docs/design/).

## Repository layout

```
boot/        Aster Boot - UEFI loader (Rust, no_std)
kernel/      Aster Kernel - higher-half x86_64 kernel (Rust, no_std + alloc)
shared/      Boot ABI and syscall ABI shared by kernel/user space
user/        Aster Desktop and built-in applications (ring 3, no_std)
tools/       Build, image, QEMU, font/icon/logo generation and test tooling
tests/       Host-side unit tests
docs/        Architecture, research, ADRs, design system, security, benchmarks
assets/      Font atlas, supplied logo, generated icon set and wallpapers
```

There are no empty scaffolding directories: every directory listed above
contains code or documentation that the build or test suite uses.

## Architecture at a glance

- **Firmware**: UEFI. Aster Boot locates the ESP, reads `KERNEL.ELF` and
  `DESK.ELF`, loads ELF PT_LOAD segments, builds the higher-half page tables
  (direct map + kernel image), fills a versioned `BootInfo`, then calls
  `ExitBootServices` and jumps to the kernel.
- **Kernel**: modular monolithic, single CPU. Kernel image at
  `0xFFFFFFFF80000000`; direct physical map at `0xFFFF800000000000`; user
  processes start at `0x0000000000400000` (below the 2 GiB small-code-model
  boundary, a documented milestone trade-off).
- **Processes**: private 4-level page tables, ring 3, NX stacks/heaps, W^X
  segments, syscall pointer validation. The scheduler preempts user threads;
  kernel threads are cooperative except idle.
- **Desktop**: one ring-3 process that owns the framebuffer and composes
  everything in software at 1280x800. It uses the kernel only through the
  syscall ABI.

Details: [docs/architecture/OVERVIEW.md](docs/architecture/OVERVIEW.md),
[KERNEL.md](docs/architecture/KERNEL.md),
[SYSTEM_SERVICES.md](docs/architecture/SYSTEM_SERVICES.md),
[IPC.md](docs/architecture/IPC.md).

## Measured performance (QEMU TCG, 1 vCPU, 512 MiB)

| Metric | Result |
|---|---|
| OVMF firmware to Aster Boot | 1.42 s |
| Aster Boot + kernel handoff | 1.62 s |
| Kernel initialised | 1.64 s |
| Desktop ready | 4.79 s |
| First compositor frame | 4.90 s |
| Frame time (1280x800 software, idle) | ~47 ms |
| Frame time with several windows open | ~80 ms |
| Memory free after desktop init | 396 MiB of 510 MiB |

Method and caveats: [docs/benchmarks/2026-11-20.md](docs/benchmarks/2026-11-20.md).
These numbers are for QEMU's software CPU emulation and are not comparable to
native hardware or to other operating systems.

## Naming

The workspace and the project title are **Aster OS** (codename Aster). The
original long-form brief used the placeholder name "NOVA OS" in its body; all
code, ABI names, and documentation use Aster. Renaming is a branding-only task
and does not affect the architecture.

## License

MIT. Third-party components and their licenses are listed in
[THIRD_PARTY.md](THIRD_PARTY.md). Aster OS does not contain Apple, Microsoft,
Linux, or macOS source code, branding, icons, or interface artwork.
