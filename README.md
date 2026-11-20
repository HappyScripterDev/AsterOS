# Aster OS

**A new generation of personal computing.**

Aster OS is an independent, from-scratch desktop operating system for x86_64.
It is **not** a Linux distribution, **not** a modified Windows or macOS, and
**not** a desktop mockup. It has its own UEFI bootloader, higher-half kernel,
memory manager, scheduler, system-call ABI, filesystem, input stack, and a
ring-3 desktop environment with a software compositor.

![Aster Desktop](docs/screenshots/files.png)

## Status: developer preview

This repository contains a working, bootable system on QEMU (UEFI + GPT/FAT32
disk image). The current milestone is a **single-CPU developer preview**:

| Area | What actually works |
|---|---|
| Boot | Aster Boot (own UEFI loader), GPT + FAT32 ESP, ELF kernel loading, custom page tables, `ExitBootServices`, higher-half handoff |
| Kernel | GDT/TSS, IDT, 256 vectors, exceptions, PIC/PIT at 1 kHz, 4-level paging, bitmap frame allocator, kernel heap (`alloc`), preemptive scheduler, ring-3 processes, syscalls via `int 0x80` |
| Memory | 4 KiB frames, 32 GiB addressable bitmap, per-process PML4, NX/W^X user mappings, heap self-tests, user copy validation |
| Drivers | COM1 serial, UEFI GOP framebuffer, PS/2 keyboard + mouse (IRQ driven), PIT |
| Filesystem | In-memory ramfs with paths, directories, files, file descriptors, `readdir`, `stat`, `unlink`, `mkdir` |
| Desktop | Aster Desktop in ring 3: wallpaper compositor, glass top bar, dock with magnification, launcher, window manager (drag, resize, maximize, snap, animations), notifications, control centre, virtual desktops |
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

- Top bar: Aster menu, focused app, clock/date, notifications, control centre.
- Dock: magnification, running indicators, tooltips, click to launch/focus.
- Launcher: `Ctrl+Space`, fuzzy app search, keyboard navigation.
- Windows: drag, eight-way resize, minimize to dock, maximize, snap to halves,
  double-click titlebar, rounded corners, cached soft shadows.
- Control centre: real dark/light theme, accent colour, motion toggle,
  brightness overlay (software), honest "No adapter" rows for Wi-Fi/Bluetooth.
- Shortcuts: `Ctrl+Space` launcher, `Ctrl+W` close, `Ctrl+M` minimize,
  `Ctrl+Tab` cycle, `Ctrl+Left/Right` virtual desktops.

Design tokens and interaction rules: [docs/design/](docs/design/).

## Repository layout

```
boot/        Aster Boot - UEFI loader (Rust, no_std)
kernel/      Aster Kernel - higher-half x86_64 kernel (Rust, no_std + alloc)
shared/      Boot ABI and syscall ABI shared by kernel/user space
user/        Aster Desktop and built-in applications (ring 3, no_std)
tools/       Reproducible build, image, QEMU, font and test tooling
tests/       Host-side unit tests
docs/        Architecture, research, ADRs, design system, security, benchmarks
assets/      Generated font atlas (OFDL font, see THIRD_PARTY.md)
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
| OVMF firmware to Aster Boot | 1.44 s |
| Aster Boot + kernel handoff | 1.62 s |
| Kernel initialised | 1.65 s |
| Desktop ready | 4.12 s |
| First compositor frame | 4.24 s |
| Steady frame time (1280x800 software) | ~54 ms (~18 fps) |
| Memory free after desktop init | 407 MiB of 510 MiB |

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
