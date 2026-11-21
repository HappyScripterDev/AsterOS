# Aster OS Roadmap

Status legend: COMPLETE, PARTIAL, NOT STARTED, BLOCKED. Every milestone names
the acceptance evidence, not just the work item.

## Release roadmap (updated 2026-11-21)

The phase sections below remain the engineering backlog. This section is the
product-level release plan; every version gets a directory under
`docs/releases/` with its baseline, test report, performance data and known
limitations.

### 0.5.0 - Graphics Engine & Motion System (COMPLETE FOR MILESTONE)

Delivered: dirty-region software compositor with partial present and idle frame
skipping; measured frame/telemetry in Monitor; Quality/Balanced/Performance/
Adaptive modes; `aster-motion` easing/tween/spring/track/sequence engine; window
state machine with animated open/close/minimize-to-dock/restore/maximize/snap;
Dock 2.0 positions/size/magnification/autohide/indicators; workspace slide;
Window Overview with captured previews; held-Alt switcher; Text tabs; menu/toast
motion; real effect switches; Graphics & Motion settings with persistence.

Not delivered: GPU acceleration, VirtIO-GPU, live backdrop blur, per-window
offscreen surfaces, occlusion culling, HiDPI runtime scaling, multi-monitor,
Stage Manager/window groups, drag-reordered tabs. These remain backlog and are
documented in `docs/releases/0.5.0/`.

### 0.6.0 - Hardware, Audio & Power

- ACPI (RSDP/XSDT/MADT/FADT), APIC/IOAPIC, MSI and interrupt-driven e1000/AHCI;
  SMP groundwork with two vCPUs.
- USB xHCI: HID keyboard/mouse, mass storage and hotplug.
- Audio: HDA, mixer, PCM, system sounds and volume in Control Center.
- Bluetooth: USB HCI, BLE and pairing UI; audio profiles once audio is stable.
- Power: shutdown, reboot, sleep, battery/thermal, lid and watchdog.
- Device Manager, hardware inventory, driver model and error reporting.
- Multi-monitor and EDID; display modes if not delivered in 0.5.

### 0.7.0 - Storage, Users & Security

- AFS v2: journal/COW, long names, permissions, snapshots and crash-injection
  tests; migration from FAT32.
- Users: accounts, login/lock screen, Argon2id, sessions and fast user switch.
- Permissions: capabilities, file ACLs, privilege prompts, network caps.
- Isolation: every app out of process, sandbox profiles, syscall filters and
  shared-memory surfaces.
- Hardening: ASLR/KASLR, SMEP/SMAP/UMIP, guard pages, quotas and audit log.
- Crypto: CSPRNG, TLS 1.3, certificate store, signatures and the loader's
  secure-boot chain.
- Firewall and network policy, DNS privacy, updated threat model and external
  review.

### 0.8.0 - Ecosystem & Developer Platform

- App ABI 2 with stability guarantees, versioned SDK, examples, templates and
  `cargo aster`.
- Out-of-process app model: surface sharing, input routing, clipboard and
  drag-and-drop between apps.
- Package format, manager, repository, signatures, dependencies and rollback;
  a real Store.
- Developer tools: UI inspector, profiler, crash reporter, log viewer and
  debug symbols.
- One real third-party port (game or emulator) as proof of graphics, audio,
  input and packaging.

### 0.9.0 - Release Candidate

- Feature freeze, installer (partitioning, bootloader install, recovery),
  updates with A/B or rollback, recovery/safe mode.
- Boot-time, memory and frame budgets on reference hardware.
- Hardware matrix (QEMU plus two or three physical machines).
- Accessibility audit (WCAG 2.2 AA), keyboard-only paths, i18n (EN/DE).
- Fuzzing (syscalls, filesystem, network), 24-hour stability and power-loss
  tests.
- Documentation, release notes, signed artifacts and security review.

### 1.0.0 - Aster OS

First complete release: desktop, kernel, networking, audio, security, SDK,
installer, updates and ecosystem. Release gate: nothing is claimed that is not
tested.

### 1.x horizon

Browser with engine integration, remote desktop/virtualization, screen reader,
voice/touch/pen input, cloud sync/backup, more GPUs and hardware video, and
additional architectures as research.

### Cross-cutting tracks

- Performance budgets per release, not only at the end.
- Accessibility from 0.5 onward; audit in 0.9.
- Security and threat-model review per release.
- Golden images, fuzzing, fault injection and CI on real runners.
- Release evidence: baseline, test report, performance and known limitations.
- Real hardware from 0.6 onward.

## Phase 0 - Research and architecture (COMPLETE)

Delivered: `docs/research/` (kernel architecture, UEFI boot, x86_64 memory and
interrupts, Rust in the kernel), `docs/decisions/0001-0004`, architecture
overview, design system, threat model, feature matrix.

Acceptance: decisions are recorded with alternatives and risks; architecture
matches the code; no unsupported claims.

## Phase 1 - Build infrastructure (COMPLETE)

Delivered: pinned Rust toolchain, `tools/build.py`, deterministic GPT/FAT32
builder with verifier, QEMU runner, firmware fetch, font atlas generator, host
tests, CI workflow.

Acceptance: a clean checkout builds with three documented commands; the image is
verified; QEMU boots it.

## Phase 2 - First boot (COMPLETE)

Delivered: Aster Boot UEFI loader, ELF loading, higher-half page tables,
BootInfo, ExitBootServices, kernel entry, serial logging, GDT/IDT, panic path.

Evidence: `BOOT-HANDOFF-OK` and `KERNEL-READY` in `build/serial.log`; boot smoke
test in `tools/test.py`.

## Version 0.3.0 - Storage and services (COMPLETE, 2026-11-20)

Delivered: block registry with capacity/bounds/flush/counters, mount table,
AHCI flush, `STORAGE_DETAILS`, `RENAME`, `SYNC`, `LOG_READ`, versioned settings
with backup and corruption recovery, the `configd` settings service with a
desktop supervisor and IPC receive timeouts, real Files operations, and
Monitor storage/log storage views.

Evidence: `docs/releases/0.3.0/TEST_REPORT.md` and `PERFORMANCE.md`.

## Phase 3 - Core kernel (PARTIAL)

Complete: physical frames, virtual memory, kernel heap, exceptions, PIC/PIT,
preemptive user threads, context switching, syscalls, ramfs, process loading,
user fault containment.

Remaining:

| Item | Acceptance |
|---|---|
| IPC channels | DONE (M3): ping-pong between isolated processes; forged handle EBADF; hangup EPIPE |
| `SPAWN`/`WAIT` | DONE (M2): shell starts children, waits, collects 42 and -11 |
| Per-process isolation hardening | Address spaces and W^X work; guard pages, SMEP/SMAP/UMIP and quotas remain |
| APIC + IOAPIC | Timer and keyboard work without the 8259; no spurious IRQs under load |
| SMP | Two vCPUs boot, schedule, and pass stress tests; per-CPU data and TLB shootdown |
| Demand paging / COW | User memory is mapped lazily; fork-like sharing is safe |

## Phase 4 - Drivers and user space (PARTIAL)

Complete: serial, GOP framebuffer, PS/2 keyboard/mouse, ramfs, desktop as PID 1.

Remaining:

| Item | Depends on | Acceptance |
|---|---|---|
| PCI enumeration | - | Enumerate q35 devices, map BARs, log vendor/device |
| ACPI tables | - | Parse RSDP/XSDT/MADT; discover CPUs and the I/O APIC |
| VirtIO block | PCI, VMM | Read/write a raw image in QEMU, verified by host comparison |
| Persistent filesystem | block driver | DONE: block registry, mount table, FAT32 home; write/reboot/read verified; no journal yet |
| AFS v2 (journal/COW) | AFS v1 | Crash-injection tests recover without corruption |
| Service manager + init | SPAWN, IPC | PARTIAL: configd supervisor restarts one service; generic manager/KILL next |

## Phase 5 - Graphics (PARTIAL, 0.5.0 delivered)

Complete: ring-3 software compositor, dirty-region renderer, partial present,
window manager state machine, Dock 2.0, top bar, launcher, Control Centre,
notifications, workspace slides, overview, quick switcher, Text tabs, motion
engine and Graphics & Motion settings.

| Item | State | Acceptance |
|---|---|---|
| Engine core (layers, dirty regions, partial present) | COMPLETE | Pixel tests; idle frames skipped; no full redraw when idle |
| Motion / window animations | COMPLETE | Time-based transitions; real opacity; host motion tests |
| Overview / Alt-Tab | COMPLETE | Real captured previews; keyboard and pointer paths tested |
| Dock autohide / positions | COMPLETE | Bottom/left/right, size, magnification, autohide, indicators |
| Glassmorphism | PARTIAL | Shadows/transparency/effects switches; blur is cached wallpaper blur |
| HiDPI and scaling | NOT STARTED | Display/DisplaySet + ui_scale foundation only |
| Stage Manager / window groups | NOT STARTED | Tabs exist in Text only; no window groups |
| Fullscreen | PARTIAL | Window maximize; no dedicated fullscreen space |
| Accessibility | PARTIAL | Reduce motion/speed/effect switches; no accessibility tree/high contrast |
| Multi-monitor | NOT STARTED | Single real display; 0.6 target |
| GPU acceleration | NOT STARTED | Software renderer only; no fake metrics |

## Phase 6 - Applications (PARTIAL)

Complete: Terminal, Files, Settings, Calculator, Monitor, Text, Notes, Calendar,
Wallpapers.

Deliberately absent: Store, Installer, Update, Browser. They require platform
capabilities that do not exist (packages, disk, network, a web engine
compatibility layer). They must not be added as mockups.

Remaining:

| App | Prerequisite | Acceptance |
|---|---|---|
| Archive | AFS/ramfs archive format | Create/extract an archive with integrity check |
| Photos | Image decoder | Open BMP/PNG from the filesystem |
| Browser | Network + TLS + engine integration | Document which engine is integrated and its license; no claim of a custom engine |
| Store | package service | Install/remove a real package with signature verification |
| Installer | AFS + partitioning | Install to a QEMU disk and boot from it |
| Update | signed metadata | Verify, stage, apply, roll back |

## Phase 7 - Networking and security

### IPv4 networking (COMPLETE for 0.4.0)

| Item | Acceptance | State |
|---|---|---|
| Ethernet + ARP + IPv4 | Ping a QEMU host on a user-mode network | DONE |
| DHCP/DNS | Obtain an address and resolve a name | DONE |
| TCP/UDP + socket API | Loopback and host echo tests | DONE |
| Network UI | Live status in menu bar, Settings, Monitor and a diagnostics app | DONE |
| IPv6 dual stack | SLAAC, ND, ICMPv6, AAAA, TCP over IPv6 | NOT STARTED |
| TLS 1.3 | CSPRNG, reviewed primitives, certificate validation | NOT STARTED |

### Users and security (NOT STARTED)

| Item | Acceptance |
|---|---|
| Users and login | Two accounts, passwords hashed with a reviewed library, sessions |
| Permissions/capabilities | File access denied across users; tests prove it |
| Network privileges | Unprivileged processes cannot reconfigure the NIC or open raw sockets |
| Sandboxing | App runs with a restricted capability set; escape tests fail |
| Package signatures | Tampered package rejected |
| Secure update | Rollback and signature failures are safe |

## Phase 8 - Advanced (NOT STARTED)

USB (xHCI), audio (HDA), NVMe/AHCI, power management, more GPUs, multi-monitor,
SDK with a stable app ABI, developer tools, hardware compatibility matrix.

## Phase 9 - Optimization and release (NOT STARTED)

Profiling, boot optimization, memory reduction, security review, hardware test
farm, installer media, release notes, signed artifacts.

## Cross-cutting risks

| Risk | Impact | Mitigation |
|---|---|---|
| Single-address-space apps | Any app bug compromises the session | Prioritise SPAWN + IPC and move apps out of the shell |
| Storage limits | 8.3 names, no journal, no permissions, single volume | VFAT names and a journal are the next storage milestone |
| Software rendering performance | Unusable on low-end emulation | Damage tracking, lazy heap, GPU path later |
| Weak security model | Cannot claim "secure OS" | Threat model, explicit gaps, phased hardening |
| Scope creep into mockups | Loss of credibility | Feature matrix requires evidence; banned fake UI |
| Glassmorphism blur cost | Frame budget collapse in software rendering | Downsample and cache the backdrop, adapt quality, offer reduce transparency |
| Stage Manager scope | Half-finished window groups | Minimal real stage model, deterministic transitions, automated probes |
| UI scale debt | Clipped layouts at 1.25/1.5/2.0 | Scale at drawing and input boundaries; golden images per scale |
| VirtIO-GPU protocol risk | No display on the new backend | Keep GOP as the default fallback; test both paths in QEMU |
| Uncommitted 0.5 work | Lost or duplicated work | Audit first, preserve the working tree, commit in logical steps |
