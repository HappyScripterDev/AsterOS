# Aster OS - Architecture Overview

Status: developer preview, single CPU, QEMU-first. This document describes the
system **as implemented**, not an aspiration. Where behaviour is incomplete it
is marked explicitly.

## 1. Design goals and non-goals

Goals:

- An independent technical foundation: own bootloader, kernel, ABI, desktop,
  filesystem, and tooling. No Linux kernel, no Linux userland, no Windows or
  macOS components.
- A small, readable privileged core with explicit interfaces.
- Memory safety where practical: Rust `no_std` kernel, `alloc` for dynamic
  structures, unsafe isolated and documented.
- A desktop that is a first-class part of the system rather than a bolted-on
  application stack.
- Reproducible builds and automated boot/UI tests on emulated hardware.

Non-goals for this milestone:

- Binary compatibility with Linux, Windows, or macOS.
- POSIX compatibility.
- Multi-user security, networking, or package management before the kernel and
  storage layers are stable.
- Competing benchmark claims against production operating systems.

## 2. Boot chain

```
UEFI firmware (OVMF on QEMU)
  |
  |  loads /EFI/BOOT/BOOTX64.EFI from the GPT/FAT32 ESP
  v
Aster Boot (boot/, Rust, x86_64-unknown-uefi)
  |  1. COM1 diagnostics, locate GOP, choose 1280x800 if available
  |  2. Locate SimpleFileSystem on the loaded image's device
  |  3. Read KERNEL.ELF and DESK.ELF
  |  4. Parse ELF64 program headers, allocate pages, copy PT_LOAD segments
  |  5. Build 4-level page tables:
  |       direct map 0..max usable RAM at 0xFFFF800000000000 (2 MiB pages, NX)
  |       kernel image at 0xFFFFFFFF80000000 (4 KiB pages, per-segment R/W/X)
  |       identity-mapped trampoline page for the CR3 switch
  |  6. Fill BootInfo (versioned, repr(C))
  |  7. ExitBootServices (retry with a fresh memory map key if required)
  |  8. Jump through the trampoline: mov cr3, pml4; jmp kernel entry
  v
Aster Kernel (kernel/, higher half)
  |  entry stub: cli, set RSP to the kernel .bss stack, call kernel_main
  |  serial logging -> boot contract validation -> GDT/TSS -> IDT -> PIC/PIT
  |  -> PMM -> VMM -> heap -> block/AHCI/GPT/mount/FAT32 -> ramfs
  |  -> scheduler -> PS/2
  |  -> spawn desktop
  v
Aster Desktop (user/asterdesk, ring 3, PID 1)
     framebuffer mapping + input + time + memory + filesystem syscalls
```

A failed stage is fatal and reported on COM1. There is no silent fallback.

### BootInfo contract

`shared/aster-abi/src/lib.rs` defines `BootInfo` (magic `ASTER001`, version 1):
direct-map offset, framebuffer description, UEFI memory map copy, loaded
modules, kernel physical range, PML4 physical address, ACPI RSDP, TSC at
handoff, command line, and a self pointer. The kernel validates magic, version,
descriptor size, and the direct map before using it, and halts otherwise.

## 3. Memory map

| Region | Base | Notes |
|---|---|---|
| User space | `0x0000_0000_0040_0000` .. `0x0000_0000_6040_0000` | Private per process. Below 2 GiB because the milestone user toolchain uses the small code model. |
| Direct physical map | `0xFFFF_8000_0000_0000` + physical | 2 MiB pages, RW, NX. All kernel physical access goes through this. |
| Kernel heap | `0xFFFF_9000_0000_0000` | Mapped on demand, 48-256 MiB limit. |
| Kernel image | `0xFFFF_FFFF_8000_0000` | ELF segments, W^X at the page level. |
| Framebuffer (user) | `0x0000_0000_5040_0000` | Mapped into the desktop process with write permission, NX. |

Kernel PML4 entries 256..512 are shared with every process address space. All
kernel-half PML4 entries that will ever be used must exist before a user PML4 is
created; the heap currently respects this by mapping its PDPT/PD/PT during
early boot. This is a documented constraint, not a general VMM feature.

## 4. Kernel components

| Component | Files | Responsibility |
|---|---|---|
| Entry/boot | `kernel/src/main.rs`, `boot.rs` | Validate BootInfo, ordered init, panic. |
| CPU tables | `arch/gdt.rs`, `arch/idt.rs` | GDT, TSS/IST, IDT gates, ISR stubs, dispatch. |
| Interrupts | `arch/pic.rs`, `arch/idt.rs`, `time.rs` | 8259 remap, PIT 1 kHz, EOI, tick counter. |
| Physical memory | `mm/pmm.rs` | Bitmap frame allocator, 4 KiB frames, 32 GiB max. |
| Virtual memory | `mm/vmm.rs` | 4-level walks, map/unmap, per-process PML4, user copy helpers. |
| Heap | `mm/heap.rs` | First-fit with coalescing over a mapped virtual region, `GlobalAlloc`. |
| Scheduler | `sched.rs` | Threads, processes, round-robin, FXSAVE context switch, preemption, reaping. |
| Syscalls | `syscall.rs` | `int 0x80` dispatch, pointer validation, errno returns. |
| Processes | `sched.rs`, `user.rs` | Spawn, wait, zombies, fault containment; see `docs/architecture/PROCESSES.md`. |
| Services | `user/configd`, `user/asterdesk/src/services.rs` | Settings service, supervisor, restart and fallback; see `docs/architecture/SYSTEM_SERVICES.md`. |
| Storage | `block.rs`, `mount.rs`, `storage/`, `drivers/ahci.rs` | Device registry, mount table, GPT + FAT32 home over AHCI; see `docs/architecture/STORAGE.md`. |
| Filesystem | `fs.rs` | VFS front-end: ramfs root, FAT32 mount at `/home/user`, inode dispatch, fd-facing operations. |
| Input | `drivers/ps2.rs` | PS/2 keyboard/mouse, event queue. |
| Clock | `arch/rtc.rs` | CMOS wall clock for the Calendar and top bar. |
| Logging | `logging.rs` | Serial + 64 KiB ring buffer. |

## 5. Execution model

- **Kernel threads**: cooperative except the idle thread. They run with
  interrupts enabled only when idle; kernel subsystems avoid blocking.
- **User threads**: preempted by the PIT every 10 ticks (10 ms). The scheduler
  runs from the timer IRQ because the interrupted context is fully saved on the
  thread's kernel stack.
- **Privilege transitions**: user -> kernel through TSS.RSP0, which is updated
  on every context switch. Syscall gates are `DPL=3`, interrupts are cleared on
  entry (interrupt gate).
- **Context switch**: callee-saved registers + full FXSAVE area. CR3 is switched
  only when the target address space differs.

## 6. System-call interface

`int 0x80`, `rax` = number, `rdi rsi rdx r10 r8 r9` = arguments, negative
`errno` on failure (`aster_abi::errno`). Implemented calls:

`EXIT WRITE GETPID YIELD SLEEP_MS TIME_MS FB_INFO FB_MAP INPUT_POLL VM_ALLOC
LOG OPEN READ WRITE_FD CLOSE SEEK READDIR STAT UNLINK MKDIR SYSTEM_INFO RTC
PROC_LIST SHUTDOWN`

Reserved but **not implemented** (return `ENOSYS`): `SPAWN`, `WAIT`,
`CHANNEL_CREATE`, `CHANNEL_SEND`, `CHANNEL_RECV`, `SET_THEME`, plus the
network and socket calls 37-46.

Every user pointer is translated through the caller's page tables and range
checked (user top). Reads/writes copy through the direct map in page-sized
chunks. There is no `copy_from_user` shortcut that trusts a pointer.

## 7. Desktop architecture

The desktop is a single ring-3 process. It owns a backbuffer and a
pre-blurred wallpaper, composes the shell, and presents with one linear copy to
the GOP framebuffer.

- **Composition**: 32-bit `0x00RRGGBB` surface; analytic anti-aliasing for
  rounded rectangles and circles; font glyph alpha from a generated atlas.
- **Glass**: panels sample a 320x200 box-blurred wallpaper upscaled once, with
  integer tint blending. Panels over windows are opaque to avoid sampling stale
  content.
- **Windows**: z-order list, titlebar dragging, eight resize edges, maximize,
  half-screen snap, minimize animation toward the dock, cached integer soft
  shadows, rounded content clipping.
- **Input**: the kernel delivers `InputEvent`s (ASCII + special keys + modifier
  bits, absolute pointer position). The shell routes events to overlays, then
  the focused window, then the dock/top bar.
- **State**: preferences are stored in `/home/user/SETTINGS.CNF`. When the
  data disk is present this is a FAT32 file on the persistent volume; without
  it the same path falls back to the volatile ramfs.

## 8. Filesystem

The root namespace is ramfs; `/home/user` is a FAT32 mount when the data disk
is present. Both back-ends expose the same path and inode operations:

- absolute path resolution, `.`/`..`, directories, files up to 4 MiB,
- per-process file descriptor table (24 entries, 0-2 reserved for console),
- `open/read/write/seek/readdir/stat/unlink/mkdir`,
- seeded `/home/user`, `/etc`, `/system`, `/var/log`, `/apps`.

The persistent volume is real FAT32 on a second QEMU disk. It supports create,
read, write, mkdir, truncate and unlink, and survives reboots (verified by the
host verifier in `tools/persistence_test.py`). It uses 8.3 names, has no
journal, and is world-readable/writable single-user storage. Details and limits:
`docs/architecture/STORAGE.md`.

## 9. Security boundaries (implemented)

- Ring 3 isolation with private PML4s.
- NX on user stacks and heaps; writable and executable are never combined for
  user segments.
- Kernel pages are supervisor-only; user tables carry U/S at every level.
- Syscall pointer validation; no kernel direct dereference of user memory.
- Boot contract validation; kernel halts on an invalid BootInfo.
- Fatal exceptions print register state instead of rebooting silently.

Not implemented: ASLR/KASLR, SMEP/SMAP, stack canaries, user authentication,
application sandboxing, quotas, signed binaries, secure boot policy, audit
logs, and a permission model beyond the single-user assumption. See
`docs/security/THREAT_MODEL.md`.

## 10. Deliberate limitations of this milestone

- Single CPU; no APIC/IOAPIC, no SMP, no TSC deadline timer.
- BIOS/legacy boot is not supported; UEFI only.
- PCI configuration, AHCI and the e1000 network driver work, but there is no
  full PCI bus inventory, no ACPI interpreter, and no USB/audio/GPU drivers.
- No demand paging, swapping, or copy-on-write; user memory is eagerly mapped.
- No kernel preemption of arbitrary kernel threads.
- User binaries must be x86_64 ELF with a fixed base; no dynamic linking.
- Desktop rendering is software-only and runs at roughly 18 fps under QEMU TCG.
