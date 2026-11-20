# ADR 0001: Kernel Architecture

Status: Accepted
Date: 2026-11-20

## Context

Aster OS is an independent x86_64 desktop operating system built from scratch. It does not
derive from Linux, Windows, or macOS code. The first milestone is a bootable system with:

- a UEFI loader and a higher-half kernel;
- ring-3 user space and a ring-3 desktop process that owns the framebuffer through a syscall;
- one CPU, a 1000 Hz PIT, an 8259 PIC, and PS/2 input;
- per-process 4-level page tables and a direct physical map.

The team is small. The kernel must be debuggable by one person, reviewable by the team, and
safe enough to run a desktop and a few test programs. It must also leave a credible path to
stronger isolation if the project later needs untrusted drivers, fault recovery, or
certification.

The architecture decision is constrained by the language choice (Rust `no_std`, see
[ADR 0003](0003-rust-first-toolchain.md)) and the boot design (see
[ADR 0002](0002-boot-and-firmware.md)).

## Decision

Adopt a **modular monolithic kernel**:

1. One ring-0 kernel image, written in Rust `no_std`, with in-kernel subsystems for memory,
   interrupts, timers, scheduling, syscalls, and the first drivers.
2. Subsystems communicate through narrow Rust traits and explicit ownership, not through a
   stable binary module ABI. There are no loadable kernel modules in v1.
3. The kernel is not Linux-derived and does not implement a Linux-compatible driver model.
   Interfaces are designed for Aster's needs.
4. The desktop compositor is software-rendered and runs in ring 3. All user-facing applications
   run in ring 3. The kernel does not render or accelerate the desktop in v1.
5. The first drivers (PIT, PIC, PS/2 keyboard, serial, framebuffer mapping) stay in the kernel.
   Complex or untrusted drivers may move to ring 3 later.
6. The kernel exposes a stable syscall ABI to user space and keeps the door open to a future
   user-space server model.
7. Syscalls use `int 0x80` initially. The ABI is designed so the entry can switch to
   `SYSCALL`/`SYSRET` later without changing user-visible semantics. APIC and SMP replace the
   8259 PIC and PIT after the single-CPU milestone.

Explicit non-goals for v1: loadable modules, SMP, demand paging, swap, a microkernel IPC ABI,
capability-based driver isolation, and a POSIX compatibility layer.

## Alternatives

### Microkernel

A minimal kernel with scheduling, IPC, address spaces, and capability management, and with
file systems, network, and drivers as user-space servers.

- Advantages: fault isolation, least privilege, a small trusted core, and a credible path to
  formal verification (seL4 is the existence proof).
- Disadvantages: the team must design an IPC ABI, capability model, server loader, service
  discovery, and restart policy before the first pixel. IPC is on every cross-component path.
  The seL4 project's own microbenchmarks show that minimal IPC is fast, but a real desktop
  service still involves multiple messages and page grants. The team has no baseline for
  tuning that design.
- Why rejected now: it front-loads the most expensive design work and delays feedback from a
  running desktop. It remains the long-term option if isolation becomes a primary requirement.

### Hybrid

A monolithic core with selected services (file system, network, GPU) moved to ring 3.

- Advantages: moves only the components that justify isolation; keeps hot paths in the kernel.
- Disadvantages: requires the IPC and capability machinery anyway, plus two mechanisms to
  maintain. Doing it before the monolithic core is stable adds a second moving part to every
  debugging session.
- Why rejected now: it is the migration path, not the starting point. The architecture is
  designed so that individual subsystems can move later.

### Unikernel

A single address space with the application linked against library OS code.

- Advantages: small images, no user/kernel transitions, good for appliances.
- Disadvantages: no isolation between multiple applications, hardware-specific images, and a
  poor fit for a general-purpose desktop that must run untrusted programs.
- Why rejected: the product is a desktop, not a single-purpose appliance. A unikernel variant
  could be a future build target for kiosks, but not the main architecture.

### Monolithic without modular boundaries

A single kernel image with subsystems that call each other freely.

- Advantages: the absolute fastest bring-up.
- Disadvantages: interface rot, untestable coupling, and a future split becomes a rewrite.
- Why rejected: the small extra cost of traits and ownership boundaries now preserves the
  migration option later.

### Exokernel or library OS

- Advantages: exposes hardware resources directly to applications; can be efficient.
- Disadvantages: requires application-level resource management and a very different user-space
  model; too research-heavy for the first milestone.
- Why rejected: outside the product scope.

## Consequences

Positive:

- The shortest path from boot to a ring-3 desktop: one build, one debugger, one crash path.
- Direct function calls on syscall, page-fault, interrupt, and scheduler paths.
- Rust's ownership and type system reduce memory-safety bugs in the kernel.
- The desktop and applications are already isolated from the kernel by ring 3 and page tables.
- The first milestone does not need an IPC ABI, capability model, or server loader.
- Subsystem traits and narrow interfaces keep a future hybrid or microkernel migration feasible.

Negative:

- The ring-0 trusted computing base is large. A driver bug, MMIO mistake, or DMA error can panic
  the whole system.
- There is no fault isolation between kernel subsystems. A failed file system or GPU driver takes
  down the desktop.
- The kernel image and its interfaces will grow; without discipline, the "modular" part erodes.
- Moving a subsystem to ring 3 later requires a real IPC design and a device-reset strategy.
- The architecture does not provide the isolation guarantees that a microkernel can offer to a
  certification authority.

Neutral:

- "Monolithic" is a spectrum. Aster is monolithic in deployment (one ring-0 image) but modular in
  source structure. The label should not be used to claim either microkernel isolation or
  unikernel simplicity.

## Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| A ring-0 driver bug panics the system | High over time | High | Narrow HAL; W^X and NX; syscall validation; fuzz the user boundary; move complex drivers to ring 3 when a trigger fires |
| Kernel TCB grows beyond review capacity | Medium | High | Size budget; module ownership; reject features that do not serve the milestone; review triggers in the research note |
| Interface rot makes a future split a rewrite | Medium | High | Traits and handles instead of shared globals; no direct access to scheduler or page-table internals from drivers |
| Single-CPU assumptions leak into global code | High | Medium | Per-CPU abstraction from the start, even with one CPU; no "current CPU" globals without a wrapper |
| Performance assumptions are wrong | Medium | Medium | Measure syscall, interrupt, and frame latency after first boot; use the triggers, not intuition |
| Memory exhaustion without demand paging | Medium | Medium | Frame accounting; fail allocations cleanly; no overcommit in v1 |
| Rust compiler or target regression | Low | Medium | Pin the toolchain; boot-test every upgrade; see ADR 0003 |

## Follow-ups

1. Define the subsystem traits and ownership rules for memory, interrupts, timers, scheduling,
   syscalls, and the first drivers. Keep them free of global state.
2. Implement the first boot path: loader handoff, GDT/TSS/IDT, physical frame allocator, kernel
   heap, per-process PML4, and the syscall entry.
3. Implement the ring-3 desktop path: framebuffer mapping through a syscall, input events, and a
   simple compositor.
4. Add the metrics needed for the review triggers: kernel source size, panic attribution, syscall
   latency, IPC volume (once IPC exists), and lock contention (once SMP exists).
5. Review this ADR after the first boot and after the first driver that is not PIT/PIC/PS/2.
6. Revisit the microkernel option only when a trigger in the research note fires; do not migrate
   on architectural taste.

## Related documents

- [Kernel architecture research](../research/kernel-architecture.md)
- [ADR 0002: Boot and firmware](0002-boot-and-firmware.md)
- [ADR 0003: Rust-first toolchain](0003-rust-first-toolchain.md)
- [ADR 0004: Address space layout](0004-address-space-layout.md)
