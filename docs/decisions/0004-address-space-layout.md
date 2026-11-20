# ADR 0004 - Address-space layout (as implemented in 0.1.0)

**Status:** Accepted for the developer preview, with a documented deviation.
**Date:** 2026-11-20.
**Supersedes:** the original target layout below, for the 0.1.0 build only.

## Implementation status (read this first)

The original ADR (kept below as design rationale) proposed user space at
`0x0000004000000000` with a 256 GiB guard. The **shipped 0.1.0 layout is**:

| Region | Start | End | Notes |
|---|---|---|---|
| User guard | `0x0000000000000000` | `0x00000000003FFFFF` | Unmapped; catches null and low pointers |
| User image/stack | `0x0000000000400000` | `0x0000000000403FFFFF` | ELF image linked at 4 MiB, 1 GiB stack window |
| User heap/mmap | `0x0000000001000000` upward | `0x000000000040400000` | `vm_alloc` cursor, below the stack |
| Framebuffer (user) | `0x000000000050400000` | + framebuffer size | Mapped by `FB_MAP`, NX, write-only |
| User top | `0x000000000060400000` | - | Validation bound in the kernel |
| HHDM | `0xFFFF800000000000` | - | 2 MiB pages, RW, NX |
| Kernel heap | `0xFFFF900000000000` | - | Grows on demand |
| Kernel image | `0xFFFFFFFF80000000` | - | ELF segments, W^X |

**Why the deviation:** the user binaries are compiled with the small code model
(the toolchain currently emits `R_X86_64_32S` relocations that cannot represent
addresses above 2 GiB). A 4 MiB base is a valid, if conservative, choice for
this milestone. Raising it requires building user space with a large code model
(or `-mcmodel=large`-equivalent Rust flags) and re-validating the ELF loader and
syscall validation bounds. This is tracked in `docs/PROJECT_STATUS.md` and
`docs/ROADMAP.md`.

**What the original ADR still governs:** the higher-half kernel base, the HHDM,
the kernel heap region, the rule that the kernel half is shared into every
process, and the requirement that user and kernel pointers are distinguishable.
Those are implemented as described below. The 256 GiB guard and the exact
`0x4000000000` base are **not** implemented in 0.1.0.

**Consequences of the actual layout:** less address-space separation between
user and kernel (the sign bit still separates them), a small low guard, and
less room for compatibility mappings. No functional feature depends on the
larger guard today.

---

# Original ADR (target layout)

# ADR 0004 - Address-space layout

**Status:** Accepted (design target; see implementation note above)
**Date:** 2026-11-20
**Deciders:** System Architect, Kernel Engineer, Security Engineer

## Context

Aster OS targets x86_64 with 4-level paging and a higher-half kernel. The
address-space split determines:

- which pointers the kernel can copy from user space,
- where the kernel image and direct physical map live,
- how much virtual address space applications can use,
- whether the kernel can use the kernel code model,
- what a future ASLR implementation can randomise.

The system must be independent of Linux, but the x86_64 architectural
constraints (48-bit canonical addresses, 4-level paging, a 2 GiB kernel code
model window) are the same for every OS.

## Decision

Use a 48-bit canonical split with the kernel in the upper half:

- user space in the lower half, starting at `0x0000004000000000` in the design
  target (4 MiB in the 0.1.0 build, see above);
- direct physical map (HHDM) at `0xFFFF800000000000`;
- kernel image at `0xFFFFFFFF80000000`.

The detailed table below is the design target.

| Region | Start | End | Size | PML4 entries | Permissions | Purpose |
| --- | --- | --- | --- | --- | --- | --- |
| User guard | `0x0000000000000000` | `0x0000003FFFFFFFFF` | 256 GiB | 0 | Unmapped | Catch NULL, wild low pointers, and integer-to-pointer bugs |
| User space | `0x0000004000000000` | `0x00007FFFFFFFFFFF` | 128 TiB - 256 GiB | 0-255 | User RX/RW, NX on data | Per-process code, data, heap, stack, mmap |
| Non-canonical hole | `0x0000800000000000` | `0xFFFF7FFFFFFFFFFF` | ~16 EiB - 256 TiB | n/a | Never mapped | Hardware canonicality boundary |
| HHDM | `0xFFFF800000000000` | `0xFFFFBFFFFFFFFFFF` | 64 TiB | 256-383 | Supervisor RW, NX | Direct map of RAM: `virt = phys + HHDM_BASE` |
| Guard | `0xFFFFC00000000000` | `0xFFFFFFFF7FFFFFFF` | 64 TiB - 2 GiB | 384-510 | Unmapped | Catch wild kernel pointers |
| Kernel image | `0xFFFFFFFF80000000` | `0xFFFFFFFFDFFFFFFF` | 1.5 GiB | 511 | Supervisor RX/RW, W^X | Text, rodata, data, bss, boot stack |
| Reserved | `0xFFFFFFFFE0000000` | `0xFFFFFFFFFFFFFFFF` | 512 MiB | 511 | Unmapped | Future kernel use |

The table partitions the 48-bit canonical address space exactly. The lower
128 TiB is a 256 GiB guard plus user space. The upper 128 TiB is a 64 TiB HHDM,
a guard of 64 TiB minus 2 GiB, and the 2 GiB kernel window. There is no overlap,
and every canonical address that the kernel does not intend to map is
explicitly left unmapped.

## Rationale

**Higher-half kernel at `0xFFFFFFFF80000000`.** This is the conventional x86_64
kernel code model by default, and Linux places its kernel text at the same
base. A fixed higher-half address lets the compiler use sign-extended 32-bit
displacements and keeps the lower half free for user space. It also makes
user/kernel pointer classification a single sign-bit test.

**HHDM at `0xFFFF800000000000`.** The kernel needs to access arbitrary physical
memory (page tables, UEFI structures, DMA buffers). A direct map with 2 MiB
pages avoids permanently consuming low virtual addresses and keeps the kernel
image independent of physical placement. 64 TiB is enough for the foreseeable
physical-memory range; 5-level paging is a separate decision.

**User base at `0x0000004000000000`.** A 256 GiB unmapped guard below user
space catches NULL dereferences, small integer-to-pointer bugs, and null-page
exploits. It also leaves the low 256 GiB free for future firmware or
compatibility mappings without colliding with user code. The base is 256 GiB
aligned (a power of two), which keeps the page-table indices simple. The cost is
256 GiB of wasted virtual address space, which is acceptable in a 128 TiB user
half but is worth documenting as a deliberate choice rather than an accident.

**Kernel image inside PML4 entry 511.** One page-table entry covers 512 GiB,
which is ample for a kernel image, modules, and future vmalloc. Keeping it in
the last entry makes the kernel window obvious and keeps the HHDM separate.

## Consequences

- User pointers are those with the high bit clear (canonical lower half). The
  syscall layer rejects anything at or above `USER_TOP` (currently
  `0x60400000`).
- The kernel can copy user memory through the HHDM after translating the user
  page, which is what `copy_from_user`/`copy_to_user` do.
- User code is linked at a fixed base in 0.1.0 (4 MiB); the design target is
  256 GiB. The ELF loader must be updated with the base.
- A user process gets its own PML4 with the kernel half copied. Kernel mappings
  are shared; user mappings are private.
- The desktop receives a separate user mapping of the framebuffer through a
  syscall; it never touches physical memory.
- ASLR is not implemented. The layout reserves room for it but every process
  loads at the same base.

## Alternatives considered

### Identity-mapped kernel at low addresses

- Advantages: simplest possible loader and early boot; no higher-half
  transition.
- Disadvantages: user and kernel pointers are ambiguous; user space cannot use
  the low addresses; the kernel is vulnerable to null-pointer and low-address
  attacks; the code model is a poor fit.
- Why rejected: fails the security and future-user-space requirements.

### User base at 4 MiB (`0x0000000000400000`)

- Advantages: the conventional ELF load address; smaller low guard.
- Disadvantages: only 4 MiB of guard; less room before the 256 GiB boundary;
  no real benefit.

Note: this is what 0.1.0 actually uses because of a toolchain constraint, not
because it is the preferred long-term choice. It will be raised once the user
toolchain can emit large-model code.

### 5-level paging

- Advantages: 57-bit virtual addresses, much larger HHDM.
- Disadvantages: not available on all target CPUs; page-table walk changes;
  more complexity for no current benefit.
- Why rejected for now: 4-level paging covers the current memory and user-space
  needs.

### Full higher-half direct map of all physical memory with 1 GiB pages

- Advantages: fewer page-table entries.
- Disadvantages: requires CPU support and 1 GiB alignment; QEMU's memory layout
  does not always align; 2 MiB pages are simpler and sufficient.
- Why rejected: 2 MiB pages are a better default.

## Risks

- The 256 GiB user guard wastes virtual address space (harmless, but worth
  documenting).
- The original user base was not achievable with the current toolchain; the
  actual 4 MiB base reduces the guard. This is a temporary, documented
  deviation.
- No ASLR for user programs; every process loads at the same base.
- The layout is x86_64-specific and assumes 4-level paging.
- Kernel mappings in every process must stay consistent; a new PML4 slot
  allocated after process creation would not be visible without an explicit
  synchronisation step. The current heap avoids this, but the invariant must be
  maintained.

## Follow-ups

1. Build user space with a large code model and raise `USER_BASE` to the design
   target.
2. Add guard pages around user stacks and kernel stacks.
3. Implement KASLR and user ASLR.
4. Add a proper vmalloc region and stop using the kernel image window for
   dynamic mappings.
5. Document the physical-memory limit (currently 32 GiB in the PMM).
