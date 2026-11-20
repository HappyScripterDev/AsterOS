# x86_64 Memory and Interrupts Research

Status: Research note supporting [ADR 0004](../decisions/0004-address-space-layout.md) and
[ADR 0002](../decisions/0002-boot-and-firmware.md)
Date: 2026-11-20
Scope: 4-level paging, NX/W^X, TSS/RSP0, IDT gate types, privilege transitions, `int 0x80`
versus `SYSCALL`/`SYSRET`, PIC versus APIC, and PIT versus HPET/TSC.

Aster OS has not been built or measured. Cycle counts and latency comparisons in this note are
qualitative unless a source is explicitly cited.

## 1. Long mode and 4-level paging

### 1.1 Mode enabling

Aster uses IA-32e mode with 4-level paging (`CR4.LA57 = 0`). The architectural conditions are:

- `CR0.PG = 1`;
- `CR4.PAE = 1`;
- `IA32_EFER.LME = 1`;
- `CR4.LA57 = 0`.

With those bits set, the processor uses 4-level paging and translates 48-bit linear addresses to
52-bit physical addresses (Intel SDM Vol. 3A, Chapter 5, Section 5.5, "4-Level Paging and 5-Level
Paging") [1]. The 48-bit limit means at most 256 TiB of linear address space, split into a lower
canonical half and a higher canonical half. Aster's user space uses the lower half and the kernel
uses the upper half.

The enable sequence matters:

1. Ensure `CR0.PG = 0`.
2. Set `CR4.PAE = 1`.
3. Set `IA32_EFER.LME = 1` (MSR `0xC0000080`, bit 8).
4. Load `CR3` with the physical address of the PML4.
5. Set `CR0.PG = 1`.
6. Reload `CS` with a 64-bit code selector (far return or far jump), then reload the data
   segments.

The processor cannot switch directly between 4-level and 5-level paging while paging is enabled;
it must clear `CR0.PG`, change `CR4.LA57`, and re-enable (Intel SDM Vol. 3A, Section 5.1.2) [1].
Aster does not use 5-level paging.

### 1.2 Paging structures

4-level paging uses four levels, each a 4 KiB table with 512 8-byte entries:

| Level | Table | Entry | Selects | Notes |
| --- | --- | --- | --- | --- |
| 4 | PML4 | PML4E | Bits 47:39 of the linear address | Top level; `CR3` points here |
| 3 | Page-Directory-Pointer Table | PDPTE | Bits 38:30 | 1 GiB region per entry |
| 2 | Page Directory | PDE | Bits 29:21 | 2 MiB region per entry; `PS=1` maps a 2 MiB page |
| 1 | Page Table | PTE | Bits 20:12 | 4 KiB page |

The physical address field in each entry points to the next table, or to the final page frame
when the entry maps a page. The low 12 bits of the linear address are the page offset. A 2 MiB
page is selected by `PDE.PS = 1`; a 1 GiB page by `PDPTE.PS = 1` (Intel SDM Vol. 3A, Section
5.5) [1]. Aster uses 4 KiB pages for user mappings, 2 MiB pages for the kernel image and HHDM
where alignment allows, and 1 GiB pages only after the mapping code is proven correct.

The page-fault error code reports `P`, `W/R`, `U/S`, `RSVD`, `I/D`, `PK`, `SS`, and `SGX` bits
(Intel SDM Vol. 3A, Chapter 5, page-fault description) [1]. `CR2` holds the faulting linear
address. Aster's page-fault handler must use both, not just `CR2`.

### 1.3 Canonical addresses

For 4-level paging, bits 63:47 of a linear address must all equal bit 47. An address that does
not satisfy this is non-canonical and causes a general-protection fault, or a stack fault for
stack accesses (Intel SDM Vol. 3A, Chapter 4, Section 4.5, "Canonicality Checking") [1]. The
non-canonical hole between the lower and upper halves must never be mapped; Aster leaves it
unmapped so that wild pointers fault instead of aliasing kernel memory.

The canonical split is why the kernel can live at `0xFFFFFFFF80000000` while user space ends at
`0x00007FFFFFFFFFFF`. The two halves are connected only through the page tables, not through a
flat address space.

### 1.4 CR3 and TLB

`CR3` holds the physical address of the PML4. With `CR4.PCIDE = 0`, bits 11:0 of `CR3` are
ignored except for `PWT`/`PCD`; with PCID enabled, bits 11:0 are the process-context identifier
(Intel SDM Vol. 3A, Section 5.5.2) [1]. Aster starts without PCID: one PML4 per process, `CR3`
reload on context switch.

TLB invalidation rules:

- `INVLPG` invalidates one page for the current address space (Intel SDM Vol. 3A, Section 5.10.4)
  [1].
- Loading `CR3` invalidates non-global entries; global entries (`PTE.G = 1` with `CR4.PGE = 1`)
  survive.
- On SMP, other processors must be shot down with an IPI after a page-table change. Aster is
  single-CPU initially, so this is a later requirement.

The OSDev paging page summarizes the same 4-level structure and notes that the processor may
cache translations in TLBs and paging-structure caches, so software must invalidate after
modifying entries [4].

### 1.5 NX and W^X

The execute-disable bit is bit 63 of a paging-structure entry. It is enabled by
`IA32_EFER.NXE` (bit 11); if NXE is 0, bit 63 is reserved and must be zero (Intel SDM Vol. 3A,
Chapter 6, Section 6.13, "Page-Level Protection and Execute-Disable Bit") [1]. When NXE is 1 and
the XD bit is 1, instruction fetch from that page causes a page fault.

W^X means every page is either writable or executable, but not both [10]. On x86, a read-only
page is still executable unless NX is set, so W^X requires both:
- `CR0.WP = 1`, so supervisor writes to read-only pages fault;
- NX set on all data, stack, heap, and MMIO pages;
- NX clear only on code pages.

Aster's permission policy:

| Region | Present | Write | NX | User | Notes |
| --- | --- | --- | --- | --- | --- |
| Kernel text | Yes | No | No | No | Execute-only in effect; read is allowed by hardware |
| Kernel rodata | Yes | No | Yes | No | Constants, boot info after validation |
| Kernel data/BSS | Yes | Yes | Yes | No | Mutable kernel state |
| Kernel heap | Yes | Yes | Yes | No | Slab allocator |
| Kernel stacks | Yes | Yes | Yes | No | Guard page below each stack |
| HHDM | Yes | Yes | Yes | No | Direct physical map |
| MMIO windows | Yes | Yes | Yes | No | Cache type set per device |
| User text | Yes | No | No | Yes | Per-process |
| User data/heap/stack | Yes | Yes | Yes | Yes | Per-process; stack guard page |

`CR0.WP` is not optional if the kernel is to respect read-only mappings. The initial kernel
should set it before enabling user mode.

SMEP and SMAP are later hardening:

- `CR4.SMEP` prevents supervisor execution of user pages.
- `CR4.SMAP` prevents supervisor data access to user pages unless `RFLAGS.AC = 1`; deliberate
  user copies must use `STAC`/`CLAC`.

Both are enumerated by CPUID and are not present on every x86_64 processor (Intel SDM Vol. 2A,
CPUID instruction) [3]. Aster checks CPUID and enables them only after the user-copy paths are
audited. The SDM documents them in the paging and protection chapters (Intel SDM Vol. 3A,
Chapter 5, Section 5.6, and Chapter 6) [1].

## 2. Aster address layout in one table

The full rationale is in [ADR 0004](../decisions/0004-address-space-layout.md). The key regions:

| Region | Start | End | Size | PML4 index | Purpose |
| --- | --- | --- | --- | --- | --- |
| User guard | `0x0000000000000000` | `0x0000003FFFFFFFFF` | 256 GiB | 0 | Unmapped; catches NULL and low pointers |
| User space | `0x0000004000000000` | `0x00007FFFFFFFFFFF` | ~128 TiB - 256 GiB | 0-255 | Per-process code, data, heap, stack |
| Non-canonical hole | `0x0000800000000000` | `0xFFFF7FFFFFFFFFFF` | ~16 EiB | n/a | Never mapped |
| HHDM | `0xFFFF800000000000` | `0xFFFFBFFFFFFFFFFF` | 64 TiB | 256-383 | Direct map of RAM |
| HHDM guard | `0xFFFFC00000000000` | `0xFFFFFFFF7FFFFFFF` | ~64 TiB - 2 GiB | 384-511 (partial) | Unmapped |
| Kernel | `0xFFFFFFFF80000000` | `0xFFFFFFFFFFFFFFFF` | 2 GiB | 511 | Kernel image, heap, stacks, MMIO, vmalloc |

The kernel base matches the top 2 GiB window used by the x86-64 kernel code model and by Linux's
kernel text mapping [11]. This is a toolchain-friendly choice: the compiler can use the kernel
code model, and the kernel is not competing with user space for low addresses.

## 3. Privilege transitions and the TSS

### 3.1 Rings and selectors

Aster uses ring 0 for the kernel and ring 3 for user programs. Rings 1 and 2 are unused. A
minimal GDT has:

| Selector | Descriptor | DPL | Use |
| --- | --- | --- | --- |
| `0x00` | Null | 0 | Required null descriptor |
| `0x08` | 64-bit kernel code | 0 | Kernel CS |
| `0x10` | Kernel data | 0 | Kernel DS/SS/ES |
| `0x18` | User data | 3 | User DS/SS/ES (selector `0x1B`) |
| `0x20` | 64-bit user code | 3 | User CS (selector `0x23`) |
| `0x28` | 64-bit TSS | 0 | Task state segment; loaded with `LTR` |

In long mode, the base and limit of CS/DS/SS are ignored for flat addressing; paging is the real
protection mechanism. The GDT still matters for privilege-level transitions, the TSS, and the
IDT's code selector.

### 3.2 The 64-bit TSS

The 64-bit TSS is not a hardware task-switch mechanism in the old IA-32 sense. It holds:

- `RSP0`, `RSP1`, `RSP2`: privilege-level stack pointers;
- `IST1` through `IST7`: interrupt stack table entries;
- reserved fields;
- the I/O map base address.

The layout is Figure 10-11, "64-Bit TSS Format", in Intel SDM Vol. 3A, Chapter 10, "Task
Management" [1]. The TSS descriptor is a system descriptor; `LTR` loads it into the task
register and marks it busy. The descriptor type is the available 64-bit TSS type, changing to
the busy type after `LTR`.

Aster uses only `RSP0` and, later, `IST1` for double fault and NMI. Hardware task switching
(`CALL` to a task gate, `IRET` to a previous task) is not used; long mode does not support task
gates (Intel SDM Vol. 3A, Section 2.1.4.1) [1].

### 3.3 Stack switching on interrupts

When an interrupt or exception transfers control from ring 3 to ring 0, the processor:

1. Reads the new `RSP` from `TSS.RSP0` (or from the IST entry selected by the IDT gate).
2. Pushes `SS`, `RSP`, `RFLAGS`, `CS`, and `RIP` onto the new stack.
3. Pushes an error code if the exception produces one.
4. Loads `CS` and `RIP` from the IDT gate.
5. Clears `IF` for an interrupt gate; leaves it unchanged for a trap gate.

This is the standard privilege transition; Intel SDM Vol. 3A, Section 6.8.5, "Stack Switching",
and Section 6.8.5.1, "Stack Switching in 64-bit Mode" [1], describe the stack switch, and Chapter
7 describes the interrupt frame and `IRET` behavior.

In 64-bit mode the processor aligns `RSP` to a 16-byte boundary on interrupt delivery, which
matters for XMM save/restore and for the SysV ABI (Intel SDM Vol. 3A, Section 7.14.2, "64-Bit
Mode Stack Frame") [1]. The ISR stub must preserve the interrupted `RSP` exactly; the interrupt
frame is not a normal function call frame.

### 3.4 SYSCALL does not switch stacks

`SYSCALL` is not an interrupt gate. It does not read `TSS.RSP0` and does not switch stacks. The
kernel entry path must:

1. `SWAPGS` to obtain the per-CPU kernel pointer (if `GS` is used that way).
2. Load the kernel stack pointer from per-CPU storage.
3. Save the user `RSP` and the user registers that the ABI does not preserve.
4. Mask interrupts if they are not already masked by `IA32_FMASK`.

The OSDev system-call page makes the same point: `SYSCALL` starts with the user stack, while
`SYSENTER` on Intel has a kernel stack set by the CPU; the `SYSCALL` path needs a CPU-local
variable and `SWAPGS` [6]. This is a common source of boot-time triple faults: the first
`SYSCALL` runs on the user stack and the kernel has no valid frame.

## 4. IDT and gate types

### 4.1 Format

In 64-bit mode the IDT has 256 entries of 16 bytes each:

| Field | Bits | Meaning |
| --- | --- | --- |
| Offset low | 15:0 | Handler address bits 15:0 |
| Selector | 31:16 | Code segment selector (kernel CS) |
| IST | 34:32 | Interrupt stack table index, 0 = none |
| Type | 43:40 | Gate type |
| DPL | 46:45 | Privilege level allowed to use `INT n` |
| P | 47 | Present |
| Offset mid | 63:48 | Handler address bits 31:16 |
| Offset high | 95:64 | Handler address bits 63:32 |
| Reserved | 127:96 | Must be zero |

The IDTR is loaded with `LIDT` and contains a 16-bit limit (size minus 1) and a 64-bit base.
The IDT itself can live anywhere in the kernel's address space; it is not required to be in low
memory. OSDev's IDT page documents the 16-byte long-mode entry and the gate fields [5].

### 4.2 Gate types

Long mode supports two gate types:

- `0xE`: 64-bit interrupt gate. Clears `IF` on entry, so further maskable interrupts are blocked
  until `IRETQ` or an explicit `STI`.
- `0xF`: 64-bit trap gate. Does not clear `IF`; used for exceptions where nested interrupts are
  acceptable.

Task gates are not supported in long mode. The old 16-bit interrupt/trap gates are not valid for
64-bit code. The DPL field controls whether a user-mode `INT n` instruction can use the gate;
hardware interrupts ignore DPL. Therefore:

- Aster's `int 0x80` gate has DPL = 3.
- All other gates have DPL = 0.
- IRQ gates use type `0xE`.
- `#BP` (vector 3) and `#OF` (vector 4) can use trap gates if the handler is re-entrant.
- `#DF` (vector 8) and NMI (vector 2) should use IST entries so that a corrupted kernel stack
  cannot turn a double fault into a triple fault.

### 4.3 Error codes

Some exceptions push an error code; most do not. The ISR stubs must normalize the stack before
calling a common Rust handler. The common error-code vectors are:

| Vector | Exception | Error code |
| --- | --- | --- |
| 8 | `#DF` Double Fault | Yes (zero) |
| 10 | `#TS` Invalid TSS | Yes |
| 11 | `#NP` Segment Not Present | Yes |
| 12 | `#SS` Stack-Segment Fault | Yes |
| 13 | `#GP` General Protection | Yes |
| 14 | `#PF` Page Fault | Yes |
| 17 | `#AC` Alignment Check | Yes (zero) |
| 21 | `#CP` Control Protection | Yes |

Vectors 0-7, 9, 15, 16, 18, 19, and 20 do not push an error code. The full table is in Intel SDM
Vol. 3A, Chapter 7, and in OSDev's IDT page [1][5]. The stubs should be generated from a single
table so that the error-code flag is not hand-maintained per vector.

### 4.4 Return

Use `IRETQ`, not `IRET`, in 64-bit mode. `IRETQ` pops `RIP`, `CS`, and `RFLAGS`, and, when
returning to a less privileged level, `RSP` and `SS` (the error code is already removed by the
stub). The SDM's Section 7.14.3, "IRET in IA-32e Mode", describes the 64-bit behavior [1]. A
mismatch between the pushed frame and the return instruction is a classic cause of silent stack
corruption.

## 5. System calls: `int 0x80` versus `SYSCALL`/`SYSRET`

### 5.1 `int 0x80`

`int n` is a software interrupt. The processor:

1. Checks the IDT gate's DPL against CPL; a user `int 0x80` requires DPL = 3.
2. Switches to the ring-0 stack from `TSS.RSP0`.
3. Pushes `SS`, `RSP`, `RFLAGS`, `CS`, `RIP`.
4. Loads `CS:RIP` from the gate.
5. Clears `IF` if the gate is an interrupt gate.

Advantages:

- Simple; no MSR programming.
- Uses the same IDT machinery as exceptions and IRQs.
- Easy to trace in a debugger.
- Works on every x86_64 processor.

Disadvantages:

- IDT lookup and privilege check.
- Full interrupt frame and stack switch.
- Slower than the fast `SYSCALL` path in principle; Aster has not measured it.

Linux traditionally used interrupt `0x80` for 32-bit system calls, and OSDev describes it as the
most common portable method [6]. For Aster's first milestone it is the right choice because the
kernel already needs a working IDT and TSS for exceptions and IRQs.

### 5.2 `SYSCALL` and `SYSRET`

`SYSCALL` is a fast system-call instruction. With FRED transitions disabled, it:

- saves the address of the next instruction in `RCX`;
- saves `RFLAGS` in `R11`;
- masks `RFLAGS` with `IA32_FMASK` (MSR `0xC0000084`);
- loads `RIP` from `IA32_LSTAR` (MSR `0xC0000082`);
- loads `CS` from bits 47:32 of `IA32_STAR` (MSR `0xC0000081`), with `SS` from the next
  selector;
- enters CPL 0;
- does **not** switch stacks.

`SYSRET` is the companion:

- loads `RIP` from `RCX`;
- loads `RFLAGS` from `R11`;
- loads `CS` from bits 63:48 of `IA32_STAR`, with `SS` from the next selector;
- returns to CPL 3.

These details are in Intel SDM Vol. 2B, the `SYSCALL` and `SYSRET` instruction pages [2]. The
OSDev system-call page adds the practical warning: the `SYSCALL` entry point still has the user
stack, so the kernel must save it and load a kernel stack via a CPU-local variable and `SWAPGS`
[6].

`SYSCALL` requires `IA32_EFER.SCE = 1`. `SYSRET` requires the return `RIP` in `RCX` to be a
canonical user address; the kernel must validate it, not trust the user value. Returning to a
non-canonical address faults at CPL 0, which is not a place where a user-triggered fault should
land.

### 5.3 Comparison

| Property | `int 0x80` | `SYSCALL`/`SYSRET` |
| --- | --- | --- |
| Setup | IDT gate with DPL 3 | `STAR`, `LSTAR`, `FMASK`, `EFER.SCE` |
| Stack switch | Yes, via `TSS.RSP0` | No; kernel must switch manually |
| Saved state | Full interrupt frame | `RCX`, `R11`; kernel saves the rest |
| Interrupt flag | Cleared by interrupt gate | Cleared by `IA32_FMASK` if configured |
| Return | `IRETQ` | `SYSRET` |
| Debuggability | High | Lower; MSR state and `SWAPGS` bugs are subtle |
| Expected performance | Slower | Faster in principle; not measured by Aster |
| Portability | All x86_64 | All x86_64, but behavior differs from 32-bit `SYSENTER` |

Aster decision: implement `int 0x80` first, with a stable syscall ABI (number in `RAX`, arguments
in `RDI`, `RSI`, `RDX`, `R10`, `R8`, `R9`, return in `RAX`). Keep the user library as the only
place that knows the trap instruction. When the kernel is stable, switch the library and the
kernel entry to `SYSCALL`/`SYSRET` without changing the ABI. This gives a simple bring-up path
and a fast path later.

## 6. PIC versus APIC

### 6.1 8259 PIC

The legacy 8259A pair provides 15 usable IRQs:

- master at I/O ports `0x20` (command) and `0x21` (data);
- slave at `0xA0` and `0xA1`;
- cascade on IRQ 2, so IRQ 2 is not available for a device;
- default BIOS vectors `0x08`-`0x0F` (master) and `0x70`-`0x77` (slave) conflict with CPU
  exceptions, so the OS must remap them, commonly to `0x20`-`0x2F`;
- end-of-interrupt is command `0x20`; a slave IRQ needs EOI to both chips;
- spurious IRQ 7 and IRQ 15 exist and must not be acknowledged with a blind EOI.

These details are documented on OSDev's 8259 PIC page [8]. The same page labels the PIC a
"historical curiosity" and recommends APIC for modern systems.

Aster v1 uses the PIC because it is simple, well documented, and sufficient for one CPU, the PIT,
and a PS/2 keyboard. It remaps the PIC to `0x20`/`0x28`, masks all lines, then unmasks only the
lines it has handlers for. The PIC is not a long-term choice.

### 6.2 APIC

The APIC consists of:

- a local APIC per CPU, memory-mapped by default at physical `0xFEE00000`, with the base
  reported by `IA32_APIC_BASE` (MSR `0x1B`);
- an I/O APIC in the chipset, memory-mapped by default at `0xFEC00000`, with a redirection table
  that routes external interrupts to vectors and CPUs;
- inter-processor interrupts (IPIs) through the local APIC's interrupt command register.

The local APIC provides the LVT timer, LINT0/LINT1, error, and spurious-interrupt registers, plus
the task-priority register (TPR) and end-of-interrupt register. The I/O APIC provides per-pin
vector, polarity, trigger mode, mask, and destination fields. OSDev's APIC page documents the
register layout, the spurious vector register, EOI, and the I/O APIC redirection table [7]; Intel
SDM Vol. 3A, Chapter 13, "Advanced Programmable Interrupt Controller (APIC)", is the normative
reference [1].

Aster's plan:

1. v1: PIC + PIT + PS/2. Keep the interrupt controller behind an `IrqController` trait.
2. v2: parse the ACPI MADT, discover the local APIC and I/O APIC, mask and disable the PIC, enable
   the local APIC, route the PIT through the I/O APIC or switch to the per-CPU APIC timer, and add
   IPIs.
3. v3: SMP, per-CPU APIC timers, and TLB shootdown IPIs.

The PIC and APIC must not both drive the same lines. OSDev explicitly warns to disable the PIC
properly when switching to APIC, including masking all interrupts and remapping, because the PIC
can still emit spurious interrupts that the kernel would misinterpret as exceptions [7].

## 7. Timers: PIT versus HPET, TSC, and APIC timer

### 7.1 PIT

The 8253/8254 PIT has a 1.193182 MHz input, three channels, and a 16-bit divisor. Channel 0 is
wired to IRQ 0. Mode 2 (rate generator) produces a periodic interrupt; mode 3 (square wave) also
works but rounds odd divisors down to even. For 1000 Hz:

```
divisor = round(1193182 / 1000) = 1193
actual frequency = 1193182 / 1193 = 1000.15 Hz
```

The PIT supports roughly 18.2 Hz to 1.19 MHz. Mode 2 is the better choice when frequency accuracy
matters; mode 3 rounds odd divisors down to even, so even divisors are preferred for a square
wave. These details are on OSDev's PIT page [9].

Aster v1 uses channel 0, mode 2, divisor 1193, IRQ 0, 1000 Hz. The tick drives the scheduler and
a monotonic millisecond counter. The counter must account for the 1000.15 Hz actual rate, not
assume exactly 1000.000 Hz, or it will drift.

Important caveat: OSDev's PIT page states that the PIT has been obsoleted by the HPET, ACPI timer,
and APIC timer, and that it has been found broken on at least one modern CPU family and absent on
at least one modern machine [9]. A 1000 Hz PIT is therefore a bring-up choice, not a production
timer. The kernel must have a fallback path before it claims support for arbitrary hardware.

### 7.2 HPET

The HPET is a memory-mapped timer complex enumerated by ACPI. It provides a higher-resolution
counter and comparators, and it is the natural next clocksource after the kernel parses ACPI.
This note did not fetch the HPET specification or OSDev HPET page successfully (the page returned
HTTP 403), so no register-level HPET details are claimed here. The PIT page's statement that the
HPET supersedes the PIT is the only HPET claim used [9].

### 7.3 TSC

The time-stamp counter is a per-logical-processor counter incremented at a constant rate on
processors that support a non-stop/invariant TSC. It is useful for elapsed-time measurement and
profiling, but it is not a replacement for a scheduler tick:

- it is not guaranteed synchronized across processors;
- its frequency must be discovered (CPUID leaves or calibration);
- it can be affected by power management on older processors;
- it does not generate interrupts.

Aster may use the TSC as a fine-grained clocksource after checking the invariant-TSC CPUID bit
(CPUID.80000007H:EDX[8]) and calibrating against the PIT or HPET. It must not use the TSC as the
only scheduling clock. The Intel SDM Vol. 2A CPUID entry documents the feature bit [3]; Vol. 3B
documents the TSC, but this note did not extract that volume, so the TSC behavior is stated as
well-established architectural detail rather than a quoted page.

### 7.4 APIC timer

The local APIC timer is per-CPU, counts down from an initial value, and can generate a periodic
or one-shot interrupt through its LVT entry. It must be calibrated against a known clock (PIT,
HPET, or TSC) before use. It is the right timer for SMP because each CPU can have its own tick.
Aster will adopt it with APIC support, not in v1.

### 7.5 Timer comparison

| Timer | Interrupt | Per-CPU | Resolution | Availability | Aster use |
| --- | --- | --- | --- | --- | --- |
| PIT | IRQ 0 | No | 16-bit divisor, ~1.19 MHz input | Legacy; may be absent or broken | v1 scheduler tick at 1000 Hz |
| HPET | Comparator | No | High | ACPI-enumerated; not universal | Later clocksource |
| TSC | None | Yes | CPU-dependent | Invariant TSC not universal | Elapsed time, calibration |
| APIC timer | LVT | Yes | Programmable | Requires local APIC | SMP tick later |

## 8. Interrupt-context rules

These are Aster project rules, not architectural requirements:

1. Do not allocate memory in an interrupt handler. The allocator may sleep or take a lock held by
   the interrupted context.
2. Do not block, sleep, or call the scheduler directly from an IRQ handler. Defer to a softirq
   or work queue after EOI.
3. Do not copy from user memory in an interrupt handler. A user page fault in interrupt context
   is not recoverable in the usual way.
4. Keep the top half short: acknowledge the device, capture the minimum state, queue work, EOI.
5. Disable interrupts around per-CPU data and while holding a spinlock that can be taken from an
   IRQ handler.
6. Send EOI after the device has been serviced, not before. For level-triggered interrupts, the
   device must deassert before EOI or the interrupt will fire again.
7. Use IST for double fault and NMI so that a stack overflow does not become a triple fault.
8. Never return from an ISR with a corrupted frame. The stubs and the common handler must agree
   on the error-code layout.

## 9. Sources and verification

Fetched successfully:

| Ref | Source | Notes |
| --- | --- | --- |
| [1] | Intel 64 and IA-32 Architectures Software Developer's Manual, Volume 3A: System Programming Guide, Part 1, order 253668-093US, September 2026: https://cdrdv2.intel.com/v1/dl/getContent/671190 | PDF downloaded and text-extracted; chapter and section names checked against the September 2026 table of contents |
| [2] | Intel SDM, Volume 2B: Instruction Set Reference, M-U, order 253667: https://cdrdv2.intel.com/v1/dl/getContent/671241 | `SYSCALL` and `SYSRET` instruction pages extracted |
| [3] | Intel SDM, Volume 2A: Instruction Set Reference, A-L, order 253666: https://cdrdv2.intel.com/v1/dl/getContent/671199 | CPUID reference; not quoted page-by-page |
| [4] | OSDev Wiki, "X86 Paging" (Wayback capture 2026-11-11): https://web.archive.org/web/20261111235951/https://wiki.osdev.org/Paging | 4-level structure, NX, canonical, INVLPG |
| [5] | OSDev Wiki, "Interrupt Descriptor Table" (Wayback capture 2026-08-31): https://web.archive.org/web/20260831163611/https://wiki.osdev.org/Interrupt_Descriptor_Table | 16-byte gate format, gate types, DPL, error-code table |
| [6] | OSDev Wiki, "System Calls" (Wayback capture 2026-09-29): https://web.archive.org/web/20260929101820/https://wiki.osdev.org/System_Calls | `int 0x80`, `SYSCALL` stack behavior, `SWAPGS` |
| [7] | OSDev Wiki, "APIC" (Wayback capture 2026-10-04): https://web.archive.org/web/20261004142302/http://wiki.osdev.org/APIC | Local/IO APIC, spurious vector, EOI, PIC disable |
| [8] | OSDev Wiki, "8259 PIC" (Wayback capture 2026-09-17): https://web.archive.org/web/20260917231522/https://wiki.osdev.org/8259_PIC | 15 IRQs, remap, EOI, spurious IRQ 7/15 |
| [9] | OSDev Wiki, "Programmable Interval Timer" (Wayback capture 2026-10-24): https://web.archive.org/web/20261024012052/http://wiki.osdev.org/Programmable_Interval_Timer | 1.193182 MHz, divisor 1193 for 1000 Hz, modern availability caveat |
| [10] | Wikipedia, "W^X": https://en.wikipedia.org/wiki/W%5EX | Definition; notes early Intel 64 chips lacked NX |
| [11] | Linux kernel documentation, "Memory Management" (x86_64 4-level map): https://docs.kernel.org/arch/x86/x86_64/mm.html | Layout comparison only |

Not verified or not fetched:

- Intel SDM Vol. 3B (time-stamp counter) was not downloaded for this note. The TSC discussion is
  limited to well-established architectural behavior; no Vol. 3B page or section number is
  claimed.
- The OSDev HPET page returned HTTP 403. No HPET register-level claims are made.
- The SDM Vol. 3A chapter numbering changed in recent revisions. This note uses the September
  2026 revision: Chapter 4 is "Linear-Address Pre-Processing", Chapter 5 is "Paging", Chapter 6
  is "Protection", Chapter 7 is "Interrupt and Exception Handling", Chapter 10 is "Task
  Management", Chapter 13 is "Advanced Programmable Interrupt Controller (APIC)". Older OSDev
  pages cite older section numbers; those were not copied.

Cross-links: [ADR 0004 address space](../decisions/0004-address-space-layout.md),
[Kernel architecture](kernel-architecture.md),
[UEFI boot](uefi-boot.md),
[Rust in the kernel](rust-in-the-kernel.md).
