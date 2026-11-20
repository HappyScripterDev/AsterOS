# Kernel Architecture Research

Status: Research note supporting [ADR 0001](../decisions/0001-kernel-architecture.md)
Date: 2026-11-20
Scope: Aster OS kernel structure, scheduling, IPC, memory management, and isolation tradeoffs.

Aster OS has not been built, booted, or benchmarked. This document is design research. Any
number marked "proposed" is an engineering policy to be calibrated later, not a measurement.

## 1. Question

Aster OS needs one kernel structure for its first bootable milestone, while keeping a credible
path to stronger isolation later. The candidate designs are:

- monolithic kernel;
- modular monolithic kernel (monolithic with in-kernel subsystems and narrow internal interfaces);
- microkernel;
- hybrid kernel (monolithic core with selected components in user space);
- unikernel (single address space, application linked with library OS code).

The decision is not "which design is best in general". It is "which design gets a small team to a
correct, debuggable, ring-3 desktop with the least irreversible commitment".

## 2. Definitions and the design space

| Design | Where most services run | Dominant cost | Dominant benefit | Role in Aster |
| --- | --- | --- | --- | --- |
| Monolithic | Kernel (ring 0) | Large trusted code base; a driver bug can panic the system | Few boundaries, direct calls, simple bring-up | Baseline, but too coarse if left unmodularized |
| Modular monolithic | Kernel, split into subsystems behind narrow interfaces | Still ring 0; interfaces can rot | Direct-call performance with reviewable boundaries | **Chosen for the first milestone** |
| Microkernel | User-space servers; kernel has scheduling, IPC, address spaces, capability management | IPC and capability bookkeeping on every cross-component operation | Fault isolation, least privilege, formal verification potential | Long-term option, not first milestone |
| Hybrid | Some services in kernel, some in user space | Two mechanisms to maintain: direct calls and IPC | Move only the components that justify isolation | Migration path if triggers are met |
| Unikernel | One address space, no user/kernel boundary | No isolation between app and OS code; hardware-specific image | Small image, low transition overhead, appliance-style deployment | Not suitable for a general-purpose multi-app desktop |

The OSDev Wiki describes a monolithic kernel as including most services in the kernel proper,
which reduces context switches and messaging but puts more code in kernel space and makes the
kernel more prone to fatal bugs [1]. Its microkernel page describes the opposite trade: services
as user-space daemons, with physical memory allocation, scheduling, and IPC left in the kernel;
it notes the messaging and context-switch cost, and that stability benefits require additional
design work such as restartable servers and recovery [2].

The same wiki's hybrid page is useful mainly as a warning about terminology. It notes that most
popular systems called "hybrid" are not clearly hybrid, and that the label is applied
inconsistently; under the definition "monolithic kernel with some components in user space",
many systems are arguably no more hybrid than ordinary monolithic kernels [3]. Aster should
therefore use "hybrid" descriptively, not as a precise category.

Unikernels are statically linked with library OS code and run as a single address space, usually
under a hypervisor. That removes repeated privilege transitions and can shrink the image, but it
also makes strong isolation between multiple applications complex and ties the image to specific
hardware or virtual hardware [7]. That is a bad default for a desktop that must run multiple
untrusted programs, but it may be useful later for a single-purpose kiosk or appliance build.

## 3. Evidence from real systems

### 3.1 Linux

Linux is the standard example of a monolithic kernel with loadable modules. The modules are still
loaded into kernel space, so the architecture remains monolithic even though the binary is not a
single unchangeable image [1]. Linux also demonstrates the value of a higher-half kernel and a
direct physical map: the x86_64 memory map places user space in the lower 128 TiB, kernel text at
`0xffffffff80000000`, and a direct mapping of physical memory in the kernel half [9]. Aster's
address-space layout borrows those proven structural ideas, not Linux code.

### 3.2 Windows

Microsoft's own overview only distinguishes user-mode and kernel-mode components; drivers call
routines exported by kernel components such as the I/O manager [8]. Windows is often called a
hybrid because it has both user-mode and kernel-mode components, but the OSDev hybrid page
explicitly lists NT/ReactOS among the examples whose classification is unclear [3]. The useful
lesson is not the label; it is that a production desktop can keep a large kernel-mode core while
moving selected policy and service code to user mode.

### 3.3 seL4

seL4 is the strongest existence proof for a microkernel with rigorous isolation. Its own
description is a minimal, policy-free microkernel with capability-based access control and formal
verification of isolation properties [5]. Its published benchmark page reports, for x86_64 on an
i7-4770 at 3.4 GHz, an IPC call of 790 cycles (std dev 5) and an IPC reply of 653 cycles (std dev
3) [6]. Those are vendor-published microbenchmarks of minimal IPC, not full service round trips,
and Aster has not reproduced them. Derived arithmetic: 790 cycles at 3.4 GHz is about 232 ns;
5,000 such calls would be about 1.16 ms, and 10,000 about 2.32 ms, against a 16.67 ms frame at
60 Hz. The raw IPC cost is therefore not automatically fatal, but the number of cross-domain
operations per frame is the real design variable.

### 3.4 Liedtke's microkernel argument

Liedtke's 1996 paper "Micro-Kernels Must And Can Be Small" argues that smallness is not an
aesthetic preference: a microkernel that is not small is not fast. Its central performance
argument is cache consumption. A frequently invoked kernel operation that touches a substantial
part of the primary cache displaces user code and data, so the user pays twice: once for the
kernel's own cache misses and again when the user working set is re-established. The paper cites
measurements on a DEC DS 5000/200 suggesting that about 20 percent of system cache misses were
caused by user-kernel competition, and argues that reducing the kernel working set can produce a
several-fold speedup for microkernel operations [10]. It also notes that first-generation
microkernels were typically around 300 KB of code and 140 system calls, and that this is the
wrong direction.

The paper's conclusion is the one used here: a microkernel that is not small, or whose IPC is
slow, loses the argument before it starts. The L4 family is the practical demonstration [2].
This is not a claim that a small microkernel automatically wins on a desktop workload; it is a
claim that size and IPC cost are first-order design constraints, not implementation details.

## 4. Tradeoffs

### 4.1 Scheduling

In a monolithic kernel there is one scheduler and one run queue abstraction. A blocking syscall
can put the calling thread to sleep directly; a wakeup can make it runnable directly; priority
inheritance for kernel locks is local to the kernel. This is simple and predictable, and it is
the right starting point for one CPU.

In a microkernel the scheduler is still in the kernel, but most work is done by user-space server
threads. A client's request blocks on IPC, the server must be scheduled, and the reply must wake
the client. Scheduling policy therefore interacts with IPC timeouts, server priorities, and
priority inheritance across address spaces. seL4's mixed-criticality scheduling support exists
precisely because those interactions are not free [5]. A desktop does not need hard real-time
guarantees on day one, but audio and input latency eventually care about this.

Aster's first milestone uses one CPU, a periodic tick, and a preemptive round-robin scheduler
with priorities. That is enough for a compositor, a shell, and a few test programs. It is not a
real-time scheduler, and the design should not claim to be one.

### 4.2 IPC

In a monolithic kernel, a subsystem call is a function call. Data can be passed by pointer if the
kernel validates user pointers, or copied with `copy_from_user`/`copy_to_user` equivalents. There
is no message format, no capability transfer, and no server lifecycle to manage.

In a microkernel, IPC is the ABI. The design must choose synchronous versus asynchronous
messages, register versus memory argument passing, shared-memory grants, capability transfer,
timeouts, and cancellation. Each choice becomes hard to change once servers exist. seL4's fastpath
shows that minimal IPC can be fast [6], but a real file read or compositor frame still involves
more than one message and often a page grant.

For a desktop, the important distinction is small control messages versus bulk data. Input events
and window-manager commands are small; framebuffers, file data, and decoded video are bulk. A
microkernel design should use shared memory for bulk data and IPC only for control. A monolithic
design gets that for free inside the kernel but must still copy safely across the user boundary.

### 4.3 Memory management

A monolithic kernel owns the physical frame allocator, kernel heap, page tables, and page cache.
It can implement copy-on-write, demand paging, and memory-mapped files with direct manipulation of
its own data structures. The cost is that all of that code is in ring 0 and all of it must be
correct.

A microkernel typically treats physical memory as capabilities: a memory manager grants frames or
mappings to servers, and the kernel tracks grant/map/unmap/revoke operations. That is powerful for
isolation, but it adds bookkeeping, revocation complexity, and TLB shootdown traffic. An external
pager can be swapped or restarted, but the protocol between pager and kernel is another ABI to
design.

Aster's first milestone: a physical frame allocator, a kernel heap, one PML4 per process, a
higher-half direct map (HHDM), and W^X page permissions. Demand paging and copy-on-write come
later. No swap. The HHDM keeps physical-to-virtual conversion trivial and avoids recursive page
tables, at the cost of a large fixed virtual window. The higher-half layout itself is the
conventional choice: OSDev's higher-half page recommends placing the kernel in the higher half,
leaving the lower half to user space, and notes that the kernel can be mapped into every process
by copying the top-level page-table mappings [4].

### 4.4 Fault isolation and drivers

This is the decisive tradeoff. In a monolithic kernel, a driver bug is a kernel bug. Rust removes
many memory-safety bugs, but it does not remove logic errors, incorrect MMIO access, DMA to the
wrong address, or a device that never completes. A bad GPU or storage driver can panic the whole
desktop.

In a microkernel, a driver server can crash and be restarted. That is only useful if the device can
be reset and its state reconstructed. GPU state recovery is notoriously hard; storage and network
devices are more tractable. The seL4 verification proves isolation properties of the kernel, not
the correctness of a driver [5]. Moving a driver out is therefore a risk-reduction technique, not
a proof of safety.

A hybrid design is the pragmatic middle: keep simple, trusted, latency-critical drivers in the
kernel (timer, interrupt controller, PS/2), and move complex or untrusted drivers (GPU, network,
USB) to user space when the isolation benefit justifies the IPC and recovery work.

## 5. Why a modular monolithic kernel is the right first milestone

1. **The boot chain is shorter.** The first milestone needs boot, physical memory, paging,
   interrupts, a scheduler, syscalls, and a ring-3 desktop. A microkernel would additionally need
   an IPC ABI, a capability model, a server loader, service discovery, and restart policy before
   the first pixel. That is a large amount of design work before any feedback.

2. **One debugger, one crash path.** A single kernel image with one log, one panic path, and one
   symbol table is dramatically easier to debug during bring-up. Distributed servers multiply the
   places where a bug can hide.

3. **Direct-call performance on the hot paths.** Syscalls, page faults, interrupts, and scheduling
   do not pay IPC or capability costs. That leaves performance headroom for the compositor and
   for the first drivers.

4. **Rust modules provide logical modularity without a runtime boundary.** Subsystems can be Rust
   modules with explicit traits and visibility, compiled into one image. This is not the same as
   hardware isolation, but it prevents accidental coupling and makes a later split cheaper.

5. **The desktop is already isolated.** "Aster Desktop" runs in ring 3 and owns the framebuffer
   through a syscall. The largest, most complex user-facing component is therefore outside the
   kernel from the start. A compositor crash need not take down the kernel if the kernel is
   designed to kill and restart the process.

6. **Single CPU keeps the concurrency model honest.** One CPU, one run queue, and a simple
   interrupt model are enough for the first milestone. SMP, per-CPU data, and cross-CPU TLB
   shootdowns can be added after the single-CPU design is correct.

7. **The option to move is preserved.** If subsystem interfaces are narrow and message-friendly,
   moving a driver or file system to ring 3 later is an engineering project, not a rewrite. The
   reverse migration, from a microkernel back to a monolith, is much harder.

The cost is explicit: Aster's first kernel is a large trusted computing base. That is acceptable
for a bring-up milestone only because the kernel is small, the team is small, and the desktop is
already outside it. It is not acceptable as a permanent security posture for untrusted third-party
drivers.

## 6. What would force a move (proposed triggers)

These are proposed review gates. They are not measurements, and the thresholds are policy, not
fact. Calibrate them after the first boot and after a stable workload exists.

| Trigger | Proposed threshold | Why it matters | First response |
| --- | --- | --- | --- |
| Driver blast radius | At least 1 system panic per 100 driver-hours attributable to one driver, or more than 20 percent of panics from one subsystem | A ring-0 driver bug kills the desktop | Move that driver to ring 3 behind a narrow device interface; keep the kernel core monolithic |
| Kernel TCB growth | More than 50k source lines (excluding tests and generated code), or more than the team can review in one release cycle | Auditability and review latency | Split a subsystem out or reduce scope before adding features |
| Syscall latency | p99 syscall plus dispatch above 5 us on target hardware, or above 2 percent of the 16.67 ms frame budget on the compositor path | Interactive latency | Optimize fast paths first; only then consider moving work out |
| IPC volume | More than 5,000 cross-domain messages per displayed frame, or more than 2 full-frame copies per frame | Microkernel overhead becomes visible | Batch, use shared memory, or keep the path in the kernel |
| SMP scaling | More than 4 CPUs and more than 20 percent of cycles in kernel lock contention under a desktop workload | Monolithic locking limits scaling | Fine-grained locking first; then consider a multikernel or microkernel structure |
| Security requirement | Need to run untrusted third-party drivers, or a certification requires least privilege for drivers | Ring-0 code is fully trusted | Hybrid driver model with IOMMU and user-space drivers |
| Availability requirement | Need to recover GPU or file system without reboot, and a prototype restart succeeds at least 99 percent of the time | Uptime | Move the recoverable service out; keep the rest monolithic |
| Team scale | More than 8 kernel engineers and more than 3 concurrent driver teams | Coordination cost and merge conflicts | Stronger internal interfaces; consider independent servers |

The thresholds are deliberately conservative. A microkernel migration is expensive; it should be
triggered by evidence, not by taste. Conversely, a single severe driver escape or a certification
requirement should override the performance argument.

## 7. Migration path if a trigger fires

1. **Freeze the interface, not the implementation.** Define block, file system, network, input,
   display, and timer interfaces as handle-based operations. Do not expose kernel pointers or
   internal locks across them.

2. **Move one component.** Start with the component that caused the trigger, usually a GPU or
   network driver. Put it in a ring-3 server with a shared-memory ring for bulk data and a small
   message protocol for control. This is a hybrid kernel.

3. **Add a capability handle layer.** User servers receive handles, not raw physical addresses.
   DMA buffers are granted through an IOMMU if available; without an IOMMU, the kernel must pin
   and validate buffers.

4. **Measure the delta.** Compare syscall latency, frame time, and CPU utilization before and
   after the move. If the regression is unacceptable, move the component back; a hybrid can be
   partial.

5. **Only then consider a microkernel.** A full microkernel means moving most services and
   accepting the IPC ABI as a long-lived interface. That is justified only if isolation,
   verification, or certification is a primary product requirement.

## 8. Decision summary

Adopt a modular monolithic kernel for the first milestone:

- one ring-0 kernel image, written in Rust `no_std`;
- in-kernel subsystems with narrow Rust traits and explicit ownership;
- no loadable modules in v1;
- ring-3 desktop and user programs;
- per-process 4-level page tables, higher-half kernel, HHDM, W^X;
- single CPU, PIT at 1000 Hz, 8259 PIC, PS/2 input;
- drivers stay in the kernel until a trigger in Section 6 fires.

This is not a claim that monolithic is safer or faster in general. It is a claim that it is the
cheapest correct first milestone for this team, and that the design keeps the exit open.

## 9. Sources and verification

Fetched successfully:

| Ref | Source | Notes |
| --- | --- | --- |
| [1] | OSDev Wiki, "Monolithic Kernel" (Wayback capture 2026-01-24): https://web.archive.org/web/20260124042501/https://wiki.osdev.org/Monolithic_Kernel | Direct wiki fetch returned HTTP 403; Wayback capture used |
| [2] | OSDev Wiki, "Microkernel" (Wayback capture 2026-10-07): https://web.archive.org/web/20261007102824/http://wiki.osdev.org/Microkernel | Community documentation, not a normative spec |
| [3] | OSDev Wiki, "Hybrid Kernel" (Wayback capture 2026-10-01): https://web.archive.org/web/20261001103404/https://wiki.osdev.org/Hybrid_Kernel | Notes terminology controversy |
| [4] | OSDev Wiki, "Higher Half Kernel" (Wayback capture 2026-09-11): https://web.archive.org/web/20260911100139/https://wiki.osdev.org/Higher_Half_Kernel | Practical layout guidance |
| [5] | seL4, "What is seL4?" and white paper page: https://sel4.systems/About/whitepaper.html | Vendor/project description |
| [6] | seL4, "Performance": https://sel4.systems/performance.html | Vendor-published microbenchmarks, generated 2026-11-17; not reproduced by Aster |
| [7] | Wikipedia, "Unikernel": https://en.wikipedia.org/wiki/Unikernel | Background, not a primary source |
| [8] | Microsoft Learn, "Overview of Windows Components": https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/overview-of-windows-components | Confirms user-mode and kernel-mode components |
| [9] | Linux kernel documentation, "Memory Management" (x86_64 4-level map): https://docs.kernel.org/arch/x86/x86_64/mm.html | Layout comparison only; no Linux code used |
| [10] | Liedtke, J., "Micro-Kernels Must And Can Be Small" (1996; original title typeset with the Greek mu): https://os.itec.kit.edu/downloads/publ_1996_liedtke_ukernels-must-be-small.pdf | PDF retrieved with curl and text-extracted; the fetch tool itself rejected the PDF content type |

Not verified by direct fetch:

- Liedtke, J. "On Micro-Kernel Construction" (SOSP 1995). Well-established bibliographic item;
  not fetched for this note. No page numbers or quotations are claimed.
- The exact 20 percent cache-miss figure is from Liedtke's citation of Chen's measurements on a
  DEC DS 5000/200; it is a historical measurement, not a measurement of Aster or of modern
  hardware.

Cross-links: [ADR 0001](../decisions/0001-kernel-architecture.md),
[UEFI boot research](uefi-boot.md),
[x86_64 memory and interrupts](x86_64-memory-and-interrupts.md),
[Rust in the kernel](rust-in-the-kernel.md).
