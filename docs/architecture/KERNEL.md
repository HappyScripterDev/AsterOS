# Aster Kernel

The kernel is a single-address-space, single-CPU, modular monolithic kernel
written in Rust (`no_std` + `alloc`). It is not derived from Linux or any other
kernel. This document covers the implemented mechanics and their invariants.

## 1. Entry and initialisation order

`kernel/src/main.rs` defines `_start` in `global_asm!`:

```
cli
cld
lea rsp, [rip + __stack_top]
xor rbp, rbp
call kernel_main
```

The loader passes `BootInfo` in `rdi` as a direct-map virtual address.
`kernel_main` performs:

1. serial + ring-buffer logging,
2. `boot::init` (magic/version/map validation),
3. CPUID vendor diagnostics,
4. GDT/TSS, IDT, PIC remap, PIT start, SSE enable,
5. PMM -> VMM -> heap,
6. heap self-test and memory self-tests,
7. block layer + AHCI, GPT, mount table and FAT32 mount + self-tests,
8. ramfs init + self-test,
9. scheduler init (idle thread), PS/2 init,
10. load `/DESK` as PID 1, `sched::start()` (never returns),
11. idle thread halts with interrupts enabled; the timer wakes runnable
    user threads.

All boot failures call `halt_with_diagnostics` or `panic!`, which writes to
COM1 and halts. Nothing continues after a failed test.

## 2. CPU tables

### GDT

| Index | Selector | Descriptor |
|---|---|---|
| 0 | 0x00 | null |
| 1 | 0x08 | kernel code, L=1, DPL0 |
| 2 | 0x10 | kernel data, D/B=1, DPL0 |
| 3 | 0x18 | user data, DPL3 |
| 4 | 0x20 | user code, L=1, DPL3 |
| 5-6 | 0x28 | 64-bit TSS (low/high) |

The TSS has no I/O bitmap. `RSP0` is updated on every context switch so a
ring-3 interrupt or syscall lands on the current thread's kernel stack. IST1
points at a dedicated 16 KiB double-fault stack.

### IDT

- 256 entries, interrupt gates (0x8E), syscall vector 0x80 with DPL=3 (0xEE).
- Exception stubs push a fake error code when the CPU does not; IRQ stubs push
  vector 0 and jump to a common handler that saves 15 registers.
- The saved frame (`InterruptFrame`) is `repr(C)`; dispatch decides whether the
  frame came from user mode via `CS & 3`.
- Stack alignment is arranged so Rust's `extern "C"` handlers see a correctly
  aligned stack for both privilege transitions.
- Double fault uses IST1, so a stack fault still produces diagnostics.

## 3. Interrupts and time

- 8259 PIC remapped to vectors 32..47. Enabled: IRQ0 timer, IRQ1 keyboard,
  IRQ2 cascade, IRQ12 mouse. All others masked.
- PIT channel 0 at 1000 Hz (divisor 1193). The tick counter is a monotonic
  `AtomicU64` of milliseconds.
- Timer IRQ: EOI, increment ticks, wake sleepers, decrement the current user
  thread's quantum, optionally `schedule()`.
- EOI is sent before scheduling; the next timer IRQ can then fire normally.
- No APIC, HPET, or TSC-deadline support yet. The TSC is read at boot for
  diagnostics only.

## 4. Physical memory manager

- 4 KiB frames; a static bitmap of 1 MiB words covers up to 32 GiB.
- Initial state: every frame above the usable end is used; UEFI
  `EfiConventionalMemory` ranges are freed; then low 1 MiB, the kernel image,
  framebuffer, BootInfo, and boot modules are reserved explicitly.
- Allocation searches from a rotating hint, never returns frame 0.
- `alloc_zeroed_frame` zeroes through the direct map.
- Accounting is exposed through `SystemInfo` (`total_memory_kib`,
  `free_memory_kib`). The framebuffer is not counted as RAM.

Known limitation: the frame bitmap is static and the maximum is 32 GiB. Larger
machines would need a dynamically sized bitmap.

## 5. Virtual memory

`mm/vmm.rs` implements 4-level x86_64 paging:

- `map_page(pml4, virt, phys, flags)` lazily allocates PDPT/PD/PT frames and
  sets U/S on **all** levels when a user page is mapped.
- `unmap_page` clears the PTE and executes `invlpg`.
- `create_user_pml4` allocates a zeroed PML4 and copies entries 256..512 from
  the kernel PML4 (direct map, kernel image, heap).
- `destroy_user_pml4` walks user entries and frees leaf frames and tables.
- `user_alloc` maps anonymous zeroed pages at a per-process cursor.
- `copy_from_user` / `copy_to_user` translate each page and copy through the
  direct map; `validate_user_range` is used for bulk buffers.

Invariants:

- The direct map is NX, RW, supervisor.
- Kernel text is RX; kernel data/BSS are RW+NX at segment granularity.
- User pages are never mapped without U/S at every level.
- User VAs are below `USER_TOP` (`0x6040_0000` in this milestone).

Limitation: all kernel-half PML4 entries must exist before the first user PML4
is created. Heap growth adds PTEs under an existing PDPT, so it is visible to
all processes, but a new PML4 slot allocated after process creation would not
be. A future VMM should synchronise kernel mappings explicitly.

## 6. Kernel heap

`mm/heap.rs` is a first-fit free-list allocator over a contiguous virtual
region at `0xFFFF_9000_0000_0000`:

- grows in 1 MiB chunks by allocating frames and mapping them RW+NX,
- 32-byte block headers, 16-byte alignment, splitting and coalescing,
- per-allocation header stores the owning block for `dealloc`,
- an interrupt-safe spin lock protects the free list,
- implements `GlobalAlloc` for `alloc::vec`, `String`, `Box`, etc.

The heap self-test allocates 256 variable-size vectors, verifies contents,
frees them, then checks that a 512 KiB allocation does not increase the
allocated-byte counter after being freed.

## 7. Threads and scheduling

`Thread` fields include tid, pid, state, kernel stack, saved RSP, CR3, user
RIP/RSP, quantum, wake tick, process pointer, and FPU state (saved on the
kernel stack via FXSAVE).

States: `Ready`, `Running`, `Blocked`, `Dead`. Round-robin from the current
thread; the idle thread is the fallback.

Context switch (`global_asm!`):

```
push rbp, rbx, r12-r15
sub rsp, 520
fxsave [aligned]
mov [rdi], rsp          ; old thread
mov rsp, rsi            ; new thread
cmp cr3, rdx; jne mov cr3, rdx
fxrstor [aligned]
add rsp, 520
pop r15..rbp
ret
```

New threads have a prepared stack whose `ret` target is either a kernel entry
function or `user_thread_entry` (which executes `iretq` to ring 3 with user
CS/SS, RFLAGS.IF=1, and the thread's user stack).

Preemption policy:

- User threads: preempted on quantum expiry (10 ms).
- Idle: preempted when a runnable thread exists.
- Other kernel threads: cooperative; they must call `yield_now`, `sleep_ms`, or
  block. This avoids preempting kernel code that holds locks during bring-up.

Reaping: dead threads' kernel stacks are freed on another thread's stack; a
process's PML4 is destroyed when its last thread dies.

## 8. Processes and ring-3 loading

`user.rs` parses a static ELF64 executable from a bootloader module:

- validates ELF64/x86_64, program header bounds, page alignment, and that
  segments lie in the image window,
- maps PT_LOAD segments with `USER`, `WRITE` if PF_W, `NX` if not PF_X,
- copies file bytes and zero-fills BSS,
- maps a 64 KiB user stack at `USER_STACK_TOP`, NX,
- registers a `Process` (pid, CR3, heap cursor, fd table) and a user thread.

Syscall entry/exit returns to the same address space and kernel stack. A user
page fault terminates only the offender: the exception handler offers the fault
to the scheduler, which logs pid/tid/vector and terminates the offending
process. Every ring-3 exception is contained, not only page faults.

## 9. Syscalls

Implemented calls and semantics are listed in
`shared/aster-abi/src/lib.rs` and `docs/architecture/OVERVIEW.md`. Key points:

- `WRITE` targets fd 1/2 (serial) only; file writes use `WRITE_FD`.
- `FB_MAP` maps the GOP framebuffer once per process at a fixed user VA with
  write-only access and NX.
- `READDIR` packs `DirEntry` records; the caller provides the entry count.
- `EXIT` marks the thread dead and schedules away permanently.
- `STORAGE_INFO`/`STORAGE_DETAILS` report mount state, device, filesystem,
  capacity and I/O counters.
- `RENAME`, `SYNC` and `LOG_READ` support Files, durable settings and Monitor.
- `SHUTDOWN` uses the QEMU ACPI poweroff ports; there is no ACPI driver yet.

`SPAWN`, `WAIT` and the IPC channel calls are implemented. `SET_THEME`
intentionally returns `ENOSYS` because the appearance is fixed to light. The
0.4.0 network calls (37-46) are implemented and listed in
`docs/architecture/SYSCALLS.md`.

## 10. Exceptions and diagnostics

Fatal exceptions print vector, error code, RIP/CS/RFLAGS, CR2 for page faults,
general registers, and then halt. Double fault has its own IST stack. The
kernel log ring buffer (64 KiB) is available for a future `dmesg` syscall; it is
not yet exposed to user space.

## 11. Known defects / risks

- User space in the low 2 GiB (small code model). Raising it requires a user
  toolchain built with a larger code model.
- No guard pages around kernel stacks or user stacks; a stack overflow will
  corrupt adjacent memory.
- Ramfs has no locking around multi-step operations beyond the global FS lock;
  it is safe on one CPU but would need finer-grained design for SMP.
- The heap and VMM are not reentrant under interrupts beyond the interrupt-safe
  lock; allocations from IRQ context are avoided by design.
- No `SMEP`/`SMAP`/`UMIP` enablement yet, so the kernel could be tricked into
  executing user pages if a kernel pointer bug existed.
- The FAT32 home uses 8.3 names, has no journal, and is world-writable. System
  files stay on the volatile ramfs. The desktop supervises configd over IPC;
  there is no generic service manager or KILL yet.
- Process teardown is still tied to the next scheduler switch; there is no
  zombie/wait model yet (M2).
