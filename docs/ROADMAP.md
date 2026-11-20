# Aster OS Roadmap

Status legend: COMPLETE, PARTIAL, NOT STARTED, BLOCKED. Every milestone names
the acceptance evidence, not just the work item.

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

## Phase 3 - Core kernel (PARTIAL)

Complete: physical frames, virtual memory, kernel heap, exceptions, PIC/PIT,
preemptive user threads, context switching, syscalls, ramfs, process loading,
user fault containment.

Remaining:

| Item | Acceptance |
|---|---|
| IPC channels | Two ring-3 processes exchange 10k messages without loss; handle forgery returns EBADF |
| `SPAWN`/`WAIT` | Shell can start a child process, wait, and collect its exit code |
| Per-process isolation hardening | Guard pages, SMEP/SMAP/UMIP, quotas |
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
| AFS v1 (read-only) | block driver | Mount a signed test image; checksums verified |
| AFS v2 (journal/COW) | AFS v1 | Crash-injection tests recover without corruption |
| Service manager + init | SPAWN, IPC | Services restart independently; desktop survives a service crash |

## Phase 5 - Graphics (PARTIAL)

Complete: ring-3 software compositor, window manager, dock, top bar, launcher,
control centre, notifications, virtual desktops, animations, dark/light themes.

Remaining:

| Item | Acceptance |
|---|---|
| Damage tracking | Idle desktop drops below 5% of one core; clock-only updates repaint only the bar |
| HiDPI and scaling | 1.25x/1.5x/2x layout without blurry text; settings control |
| Accessibility | Visible focus for every control, reduced motion, high-contrast theme, keyboard-only navigation |
| Multi-monitor | Two GOP displays (or one display + dummy) with independent work areas |
| GPU acceleration | VirtIO-GPU or Intel/AMD driver with a 2D path; software fallback retained |

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

## Phase 7 - Networking and security (NOT STARTED)

| Item | Acceptance |
|---|---|
| Ethernet + ARP + IPv4 | Ping a QEMU host on a user-mode network |
| DHCP/DNS | Obtain an address and resolve a name |
| TCP/UDP + socket API | Loopback and host echo tests |
| Users and login | Two accounts, passwords hashed with a reviewed library, sessions |
| Permissions/capabilities | File access denied across users; tests prove it |
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
| No persistence | Users lose work; cannot install anything | AFS + block driver are the next storage milestone |
| Software rendering performance | Unusable on low-end emulation | Damage tracking, lazy heap, GPU path later |
| Weak security model | Cannot claim "secure OS" | Threat model, explicit gaps, phased hardening |
| Scope creep into mockups | Loss of credibility | Feature matrix requires evidence; banned fake UI |
