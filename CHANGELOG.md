# Changelog

## 0.2.0 - Core milestone (2026-11-20)

- Added PCI configuration access and a polling AHCI driver for the q35 SATA
  controller (IDENTIFY plus single-sector DMA reads and writes).
- Added GPT parsing with header CRC validation and a separate 512 MiB data
  disk (`build/data.img`, created once and preserved across builds).
- Added a FAT32 driver: mount, lookup, read, write, create, mkdir, truncate,
  recursive unlink, directory growth and both FAT copies.
- Mounted the volume at `/home/user` through the existing VFS; the boot disk is
  only ever read.
- Settings > Storage now reports the real mount state, capacity and free space
  through a new `STORAGE_INFO` syscall.
- Preferences, Notes, Documents and user files persist across reboots; the
  persistence test verifies the guest-written volume with the host FAT parser.
- Hardened the kernel: all user exceptions are contained, PMM frame bounds are
  checked, partial mappings roll back, the kernel heap coalesces growth chunks
  and supports 1 MiB allocations, user copies require user/write permissions,
  anonymous frames carry an ownership bit, and NX/WP are enabled.
- Added missing-disk and corrupt-volume handling; both boot to the desktop.
- Implemented `SPAWN` and `WAIT`: a process gets a private address space and
  an exit status that its parent collects from a zombie.
- Contained every ring-3 exception, not only page faults; a faulting child
  returns status -11 while the desktop continues.
- Added boot-time address-space isolation tests and `tools/isolation_test.py`
  with `isoprobe` (private page, exit 42) and `faultprobe` (kernel write).
- Hardened the ELF loader: checked arithmetic, W^X, entry-in-executable and
  overlapping-segment rejection.
- Added native IPC channels: `CHANNEL_CREATE/SEND/RECV/CLOSE`, per-process
  handles, bounded queues, blocking send/receive, wake-up and close/hangup.
- Added a bootstrap handle to `SPAWN`, so a parent and child can communicate
  immediately; verified with `ipcprobe` (ping-pong) and `ipchup` (forged handle,
  hangup).

## Unreleased - light macOS-inspired redesign

- Replaced the dark/light theme system with a single light appearance. There
  is no dark mode or theme switch anywhere in the UI.
- New menu bar: supplied Aster logo tile, app name, working File/Edit/View/
  Window/Help menus, Spotlight, Control Centre, Wi-Fi status and clock.
- Spotlight-style launcher with translucent panel, filtered results, keyboard
  navigation and dynamic height.
- Redesigned nine original squircle app icons with gradients, shadows and
  app-specific artwork (`tools/icongen.py`).
- New light procedural wallpapers: Sonoma, Sequoia, Ventura and Monterey.
- macOS-style window chrome: traffic lights, centred titles, light title bars,
  softer shadows and rounded corners.
- Control Centre redesigned with connectivity tiles, display brightness,
  reduce motion and an explicit light appearance row.
- Notification centre and toasts redesigned around the supplied logo.
- About page uses the supplied logo on a branded card.
- Finder-style file and folder icons in Files.
- Archived the dark palette; design documentation now describes the shipped
  light tokens and geometry.

## 0.1.0 - 2026-11-20 (developer preview)

First bootable, testable Aster OS milestone.

### Boot

- Aster Boot: custom UEFI loader written in Rust, no GRUB.
- GPT + FAT32 ESP image generated and verified by `tools/image.py`.
- ELF kernel loading, higher-half page tables, direct physical map,
  versioned `BootInfo`, `ExitBootServices`, CR3 trampoline.
- Deterministic image builder with fixed GUIDs and FAT timestamps.

### Kernel

- Higher-half x86_64 kernel with serial diagnostics and panic reporting.
- GDT/TSS (IST double-fault stack), 256-entry IDT, exception dispatch.
- 8259 PIC remap, PIT at 1 kHz, preemptive user scheduling.
- Bitmap physical frame allocator (32 GiB addressable), 4-level paging with
  per-process PML4, NX/W^X user mappings.
- First-fit kernel heap with `GlobalAlloc`, coalescing, and a boot self-test.
- Threads, processes, FXSAVE context switching, sleep/wake, process reaping.
- Ring-3 ELF loader (static x86_64 executable, 64 KiB stack).
- 24 syscalls: console, framebuffer, input, memory, files, time, RTC, process
  info, shutdown.
- Ramfs with directories, absolute paths, fd table, readdir/stat/unlink/mkdir,
  and a self-test.
- PS/2 keyboard/mouse IRQ drivers with ASCII + special keys and modifiers.
- CMOS RTC.

### Desktop

- Ring-3 Aster Desktop with a software compositor at 1280x800.
- Procedural wallpapers, pre-blurred glass panels, rounded windows, cached
  soft shadows, animations.
- Top bar (menu, status, clock), dock (magnification, indicators, tooltips),
  launcher with search and keyboard navigation.
- Window manager: drag, eight-way resize, maximize, half-screen snap,
  minimize-to-dock, close, focus stacking, three virtual desktops.
- Control centre with working dark/light theme, accent colour, wallpaper,
  animation speed, reduced motion, brightness overlay, and honest unavailable
  Wi-Fi/Bluetooth rows.
- Notifications with history.
- Apps: Terminal (real ramfs shell), Files, Settings, Calculator, Monitor,
  Text, Notes, Calendar, Wallpapers.
- Inter + JetBrains Mono rasterized font atlas (OFL).

### Tooling and documentation

- Reproducible build (`tools/build.py`), QEMU runner, firmware fetch, font
  generator, benchmark, LOC counter.
- Host image tests, boot smoke test, scripted UI, app, pointer, and WM probes.
- Architecture, research, ADRs, design system, security threat model,
  feature matrix, status, roadmap, benchmark report.
- CI workflow (GitHub Actions) for host tests, build, and boot smoke.

### Known limitations

- No on-disk filesystem or persistence; ramfs only.
- No networking, audio, USB, storage, GPU, APIC/SMP, or ACPI.
- No IPC, `SPAWN`, `WAIT`, users, sandboxing, ASLR, or secure updates.
- Store/Installer/Update/Browser are deliberately absent.
- Built-in apps share the desktop address space.
- Software rendering runs at about 18 fps under QEMU TCG.
- User space is linked in the low 2 GiB due to the current code model.
