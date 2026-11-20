# Changelog

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
