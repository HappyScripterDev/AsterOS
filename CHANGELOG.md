# Changelog

## 0.5.0 - Graphics and motion (2026-11-21)

- Added the `aster-motion` crate: easing curves, tweens, springs, motion tracks,
  sequences and host unit tests.
- Added dirty-region compositing with bounded merging, partial wallpaper
  restore, partial framebuffer present and idle frame skipping.
- Added measured frame timing, FPS, draw/present time, dirty pixels, redraws and
  skipped frames; Monitor now shows real software-renderer telemetry.
- Added Quality/Balanced/Performance/Adaptive modes that change shadow quality
  and dirty-region limits; Adaptive reacts to measured frame time.
- Replaced timestamp effects with an explicit window state machine and
  time-based open/close/minimize-to-dock/restore/maximize/snap/focus
  transitions, including real opacity fading.
- Added Dock 2.0: bottom/left/right positions, size, magnification, auto-hide,
  running and minimized indicators.
- Added workspace slide transitions and indicators, Window Overview with real
  captured previews, and a held-Alt quick switcher.
- Added Text editor tabs with create/select/close, keyboard shortcuts and
  preserved per-tab state.
- Added menu open/close fade, toast slide/fade, reduced-motion handling and
  effect switches for shadows, transparency and wallpaper blur.
- Added Settings > Graphics & Motion with real, persisted controls and a live
  motion preview.
- Added `tools/graphics_test.py` with pixel-level assertions for minimize/
  restore, overview dimming, switcher visibility, workspace switching, tabs and
  settings persistence.
- Version strings updated to 0.5.0.

## 0.4.0 - Networking (2026-11-21)

- Added a real Intel 82540EM (e1000) driver: PCI discovery, MMIO BAR, EEPROM
  MAC, link state, 256-entry RX ring, 32-entry TX ring, synchronous TX and
  RX/TX/error counters. NIC interrupts are masked because the kernel still uses
  the PIC only; a kernel thread polls at 100 Hz.
- Added Ethernet II, ARP, IPv4, ICMP, UDP, DHCP client, DNS client and a TCP
  client with handshake, sliding send window, partial in-order receive,
  retransmission, FIN and RST.
- Added socket syscalls 37-46 (`NET_STATUS`, `NET_CONFIG`, `NET_PING`,
  `NET_DNS`, `SOCKET`, `BIND`, `CONNECT`, `SEND`, `RECV`, `SOCKET_CLOSE`),
  new errno values and `NetStatus`/`NetConfig` ABI structures.
- Added a 100 Hz kernel network thread and a per-process socket table with
  owner cleanup on process exit.
- Added live network status to the menu bar, Control Centre, Settings and
  Monitor; added the Network diagnostics app (ping, DNS, DHCP renew) and
  Terminal commands `net`, `ping` and `dns`.
- Added `tools/net_test.py`, which boots with an explicit e1000 and host-side
  TCP/UDP/DNS servers and verifies DHCP, ICMP, TCP echo, a 64 KiB TCP bulk
  echo, UDP echo and DNS. Added `tools/network_ui_probe.py` for the desktop
  integration.
- Boot smoke now requires e1000 detection and a DHCP lease. The bootloader
  loads `NETPROBE.ELF`.
- Documented architecture, measurements, limitations, and the IPv6/TLS design
  in `docs/releases/0.4.0/`.

## 0.3.0 - Storage and services (2026-11-20)

- Added a block-device registry with capacity checks, flush (AHCI FLUSH CACHE
  EXT), I/O counters and normalised errors; GPT/FAT now go through it.
- Added a mount table with longest-prefix routing, device/partition identity,
  root ramfs and the FAT32 home. Removed the hardcoded `/home/user` route.
- Added `STORAGE_DETAILS`, `RENAME`, `SYNC` and `LOG_READ`; Settings shows the
  device, filesystem, used/free space, partition and real I/O counters.
- Fixed multi-sector FAT writes repeating the first 512-byte sector; a 64 KiB
  pattern test now verifies every byte.
- Settings are versioned (`version=1`), validated per key, written through a
  temporary file with `SETTINGS.BAK`, and recover from corruption as
  `SETTINGS.BAD`. Legacy 0.2 files migrate instead of being discarded.
- Added `configd`, a ring-3 settings service over IPC, plus a desktop
  supervisor with restart, reaping and direct-I/O fallback. IPC receive now
  supports timeouts (`ETIMEDOUT`).
- Files now has forward/back, breadcrumbs, Downloads/Desktop, new folder,
  rename, delete confirmation, copy/move/paste, information, sorting and
  search, sort and forward/back history, all using real syscalls.
- Monitor shows storage usage/free, zombie state and a kernel-log view.
- Added settings corruption, service restart, Files operation, filesystem and
  block self-tests; documented performance, storage, services and limits.

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
