# Rust in the Kernel

Status: Research note supporting [ADR 0003](../decisions/0003-rust-first-toolchain.md)
Date: 2026-11-20
Scope: `no_std`, `alloc`, `panic=abort`, `global_asm!`, unsafe discipline, stable toolchain,
Rust-only linking with `rust-lld`, and known gaps versus C.

Aster OS has not been compiled or booted. This note is about language and toolchain properties,
not about measured code size or performance.

## 1. Why Rust for a new kernel

Rust offers three properties that matter for a new ring-0 code base:

1. **Memory safety by default.** Safe Rust cannot produce dangling references, use-after-free,
   double-free, or data races through ordinary references. The compiler enforces ownership and
   borrowing; the remaining unsafe operations are explicit and auditable.
2. **Expressive sum types and ownership.** Page-table entries, memory types, syscall results, and
   device states are naturally modeled as enums and newtypes. This reduces the "integer with a
   secret meaning" style that is common in C kernels.
3. **No garbage collector or runtime.** `core` is usable in a freestanding environment; there is
   no hidden runtime thread, no GC pause, and no libc dependency.

None of this proves the kernel correct. Rust's guarantees stop at `unsafe`, and a kernel is
mostly `unsafe` at the hardware boundary. The benefit is that the unsafe surface can be kept
small and reviewed, instead of being the whole code base.

The Rustonomicon describes Rust as containing both a safe and an unsafe language: safe Rust is
the true language, while unsafe Rust permits extra operations under programmer responsibility
[8]. The Reference is explicit that `unsafe` does not change the rules: code inside an `unsafe`
block must still avoid undefined behavior, and the programmer is responsible for soundness [7].

## 2. `no_std`, `core`, and `alloc`

### 2.1 What `no_std` removes

`#![no_std]` makes a crate link to `core` instead of `std`. The Embedded Rust Book summarizes the
consequences:

- no libstd and no hosted environment;
- no runtime that sets up stack overflow protection or calls `main`;
- no heap unless the `alloc` crate and a global allocator are supplied;
- `core` still provides language primitives, slices, strings, floats, atomics, and SIMD types
  [3].

For a kernel this is the correct starting point: the kernel defines its own entry point, stack,
allocator, and panic behavior.

### 2.2 `alloc`

The `alloc` crate provides `Box`, `Rc`, `Arc`, `Vec`, `String`, and the collections, plus the
global allocator interface [9]. It is not available automatically in `no_std`; the kernel must
provide a `#[global_allocator]` implementation.

Aster's plan:

- `alloc` is enabled only after the physical frame allocator and kernel heap are initialized.
- The global allocator is a slab allocator backed by the kernel heap.
- Allocation failure is handled explicitly in early boot. `alloc`'s default `handle_alloc_error`
  aborts, which is acceptable only after the heap is known to be alive.
- No allocation in interrupt context, in the scheduler's context-switch path, or while holding a
  lock that an interrupt handler can take.

### 2.3 Target properties

`x86_64-unknown-none` is the kernel target. The rustc book lists it as Tier 2 and documents:

- freestanding/bare-metal x86-64, ELF output;
- no `std` support;
- no default allocator, but `alloc` is possible with a supplied allocator;
- by default, generated code does not use vector or floating-point registers (SSE, AVX), which
  lets the kernel avoid saving them;
- the `kernel` code model by default;
- `extern "C"` uses the System V calling convention without a red zone;
- precompiled artifacts are available via `rustup target add x86_64-unknown-none` [1].

The no-red-zone property is important: interrupt delivery uses the current stack, and a red zone
below `RSP` could be clobbered by an interrupt. The kernel code model matches the higher-half
address layout in [ADR 0004](../decisions/0004-address-space-layout.md).

The UEFI loader target is `x86_64-unknown-uefi`. The rustc platform support table lists it as a
Tier 2 target without host tools, with standard-library support marked as work in progress [2].
Aster Boot uses `#![no_std]` plus `core`, so the incomplete `std` status does not matter.

## 3. Build and link model

### 3.1 Stable toolchain

Aster uses the stable release channel, pinned to an exact version. The Rust Book explains the
release train: nightly, beta, and stable, with a new stable release every six weeks; unstable
features are gated behind feature flags and are only usable on nightly [10]. A kernel should not
depend on nightly-only features for its core:

- nightly features can change or be removed;
- a compiler update can change code generation or diagnostics;
- the project needs reproducible builds more than it needs the newest feature.

Nightly may be used for experiments, but not for the shipping kernel. Upgrades are deliberate:
change the pin, run the boot and syscall tests, compare code size and boot time, then merge.

### 3.2 `panic = abort`

The kernel sets `panic = "abort"`. rustc's codegen options define `abort` as "terminate the
process upon panic", as opposed to `unwind` [4]. For a kernel this means:

- no unwinder, no `catch_unwind`;
- a panic is a fatal kernel bug, not a recoverable condition;
- the panic handler logs the location and message to serial, then halts all CPUs (one CPU in
  v1);
- `Drop` destructors do not run during a panic. Resources that must be released on a panic path
  are not relied upon.

This is a deliberate trade: unwinding in a kernel is hard to make correct, and a panic usually
means the kernel state is already suspect. User-process failures must be handled as `Result`,
not as panics.

### 3.3 Assembly

The Rust Reference documents three assembly forms, all stable on x86 and x86-64 [5]:

- `asm!` for inline assembly inside a function;
- `naked_asm!` for a naked function whose entire body is assembly;
- `global_asm!` for module-level assembly, used to hand-write whole functions.

`global_asm!` is outside a function, and `naked_asm!` defines a whole function without
compiler-generated operand handling; both are restricted to `sym` and `const` operands [5]. On
x86, the default syntax is LLVM's internal assembler in `.intel_syntax noprefix` mode, not AT&T
syntax [5].

Aster uses assembly only where the architecture requires it:

- `_start` / trampoline entry;
- `LGDT`, `LIDT`, `LTR`;
- ISR stubs for vectors with and without error codes;
- context switch (save callee-saved registers, switch stack, restore);
- a few instructions with no stable Rust intrinsic (`INVLPG`, `SWAPGS`, `STAC`/`CLAC`, `HLT`).

Everything else is Rust. There is no NASM file and no separate assembler pass.

### 3.4 `rust-lld` only

The linker is `rust-lld`, the LLVM linker shipped with the Rust toolchain. rustc exposes
`-C linker-flavor=ld.lld` and `-C linker=<path>` [4]. The kernel uses:

- `-C linker-flavor=ld.lld`;
- `-C linker=<path to rust-lld>` if `rust-lld` is not on `PATH`;
- a project linker script that places the kernel at `0xFFFFFFFF80000000` and sets the physical
  load address (LMA) for the loader;
- `-C relocation-model=static` and `-C code-model=kernel`;
- `-C panic=abort`;
- `-C opt-level=s` or `z`, LTO, and `codegen-units=1` for size and determinism.

No clang, no GNU ld, no NASM. The UEFI loader is a PE32+ image; `rust-lld` can produce it for
the `x86_64-unknown-uefi` target. The kernel is ELF64; the loader parses ELF, so no `objcopy`
step is required for the first milestone. If a raw binary format is needed later, that is a new
tooling decision, not an assumption.

## 4. Unsafe discipline

### 4.1 The rule set

The Reference's undefined-behavior list includes data races, dangling or misaligned accesses,
breaking pointer aliasing rules, producing invalid values, calling a function with the wrong ABI,
and incorrect inline assembly [7]. The following project rules exist to keep those out of Aster:

1. **Every `unsafe` block has a `// SAFETY:` comment.** The comment states the invariant, the
   preconditions, and why the surrounding code upholds them. A comment that says only "trust me"
   is not acceptable.
2. **Unsafe is confined to named modules.** `arch`, `mm::paging`, `mm::frame`, `sync`, `hal`,
   and the syscall entry stubs may use `unsafe`. Other modules expose safe APIs and do not use
   `unsafe` directly.
3. **`unsafe fn` bodies use explicit `unsafe` blocks.** The crate denies
   `unsafe_op_in_unsafe_fn`, so an unsafe operation inside an unsafe function still needs its own
   block and comment.
4. **No `static mut`.** Mutable statics are `UnsafeCell` behind a lock, or atomics. The crate
   denies `static_mut_refs`.
5. **No references to MMIO.** Device registers are raw pointers accessed with `read_volatile` and
   `write_volatile`; taking `&T` or `&mut T` to MMIO is forbidden because the compiler may
   assume normal memory semantics.
6. **Page tables are owned.** Mapping functions take `&mut PageTable` or a lock guard. The frame
   allocator is the only source of page-table frames. A physical frame is not mapped twice with
   conflicting permissions.
7. **Interrupt-disable is a guard.** `without_interrupts(|| ...)` returns a guard; dropping it
   restores the previous `IF` state. Blocking or allocating while the guard is held is a bug.
8. **User pointers are untrusted.** `copy_from_user` and `copy_to_user` validate the range,
   canonicality, mapping, and write permission, and return `Result`. A bad pointer returns an
   error; it does not panic and does not fault in kernel mode.
9. **ABI types are `#[repr(C)]` or `#[repr(transparent)]`.** Rust's default representation gives
   no layout guarantees beyond soundness [6]. Boot info, syscall arguments, and UEFI structures
   are C-layout types.
10. **Panics are fatal.** No `unwrap`/`expect` on user-controlled input; no indexing that can
    panic on a user length; no arithmetic that can overflow in release without an explicit
    checked operation.
11. **Inline assembly declares its effects.** Clobbers are listed, `options(nostack)` is used
    only when the assembly does not touch the stack, `options(nomem)` only when it does not
    access memory, and `preserves_flags` only when flags are actually preserved. The stack
    pointer is restored before returning.
12. **New unsafe code needs review.** A second reviewer reads every new `unsafe` block. If the
    block cannot be tested directly, the PR explains why and what invariant was reasoned about.

### 4.2 Soundness and safe wrappers

The pattern is always the same: a small `unsafe` implementation with a documented invariant,
behind a safe API that cannot be misused by safe code. Examples:

- `PhysAddr` and `VirtAddr` newtypes reject misaligned or out-of-range values at construction.
- `PageTable::map` returns `Result` and refuses to overwrite an existing mapping unless the
  caller explicitly asks for `remap`.
- `SpinLock<T>` hands out a guard; the lock disables interrupts on the local CPU while held.
- `UserSlice` validates the user range once and exposes only `read`/`write` methods that return
  `Result`.

The Rustonomicon's definition of soundness is the useful test: unsafe code is sound if no safe
client can use it to trigger undefined behavior [7][8].

### 4.3 What Rust does not protect against

- MMIO writes to the wrong register;
- DMA to an address the device does not own;
- a device that never completes a transaction;
- logic errors in the scheduler or page-table code;
- deadlocks, livelocks, and priority inversion;
- side channels;
- compiler or CPU bugs.

These require design review, hardware documentation, timeouts, and tests. Rust is not a
replacement for those.

## 5. Known gaps and limitations versus C

| Area | Rust reality | Consequence for Aster |
| --- | --- | --- |
| Unwinding | `panic = abort`; no unwinder; `catch_unwind` unavailable | Panics are fatal; use `Result` for recoverable errors |
| Stable ABI | `repr(Rust)` has no stable layout; ABI is not stable across compilers | Use `repr(C)`/`repr(transparent)` at every boundary |
| Vendor intrinsics | `core::arch` exposes many intrinsics, but availability is target-feature gated and not every instruction has a stable wrapper | Inline assembly is the stable escape hatch; keep it isolated |
| Target tier | `x86_64-unknown-none` is Tier 2 | Not all Rust tests run; pin the compiler and test boot on every upgrade |
| Runtime | No `std` runtime, no stack guard, no `main` | Kernel must set up its own stack, IDT, allocator, and panic path |
| SIMD/FPU | Target avoids vector/FP registers by default | Enabling SSE/AVX requires explicit context save/restore; not in v1 |
| Code size | Monomorphization can bloat; generics instantiate per type | Use `opt-level=s/z`, LTO, `codegen-units=1`; measure the image |
| Tooling | No kernel sanitizer or Miri for bare-metal code | Host-test pure algorithms; use QEMU and serial logs for the rest |
| Ecosystem | Fewer `no_std` crates; no `std::sync` | Keep dependencies minimal; implement locks and queues in-tree |
| Debug info | DWARF works, but no kernel debugger yet | Add QEMU gdb stub and symbol loading as a follow-up |

The "no stable ABI" point is often misunderstood. Rust's internal ABI is not a stable contract
across compiler versions, and `repr(Rust)` does not guarantee field order or padding [6]. That is
fine inside the kernel, where everything is compiled together. It is not fine at the syscall,
boot-info, or UEFI boundary, where `repr(C)` is mandatory.

The "vendor intrinsics" point is not a blocker. On x86-64, the instructions Aster needs for
entry, paging, and interrupts are available through `asm!`/`global_asm!`, which are stable [5].
`core::arch` intrinsics are useful for optimized routines later, but the kernel does not depend
on a complete intrinsic surface.

## 6. Toolchain policy and the C/C++ boundary

### 6.1 Policy

- Stable Rust, pinned in `rust-toolchain.toml` (a follow-up file, not created by this note).
- Targets: `x86_64-unknown-none` for the kernel, `x86_64-unknown-uefi` for the loader.
- Components: `rustfmt`, `clippy`, `rust-src` only if a custom target is introduced. The shipped
  targets avoid `-Z build-std`, which is nightly-only.
- Linker: `rust-lld` only. No clang, no GNU ld, no NASM.
- Assembly: `global_asm!`, `asm!`, `naked_asm!` only.
- No external crates in the kernel without a review of `unsafe`, licensing, and `no_std`
  support. The default is zero dependencies.
- CI builds the kernel and loader on the pinned toolchain, checks `clippy -D warnings`, and
  boots the OVMF image under QEMU.

### 6.2 Where C or C++ would be justified later

C or C++ is not banned for ideological reasons. It is a tool with a different risk profile. It
would be justified only for:

1. **Vendor driver code** that has no Rust equivalent and cannot be reasonably rewritten, such as
   a GPU or network firmware interface.
2. **Certified cryptographic code** whose certification package is tied to a specific C
   implementation.
3. **A performance-critical routine with an existing, well-tested C implementation** where a
   Rust rewrite would add risk without a clear benefit.
4. **UEFI reference code** used to validate a protocol implementation, kept out of the shipping
   loader.

Conditions for any C/C++ component:

- isolated in its own crate or static library, behind a C ABI;
- compiled freestanding, with no libc, no exceptions, and no unwinding;
- linked with the same `rust-lld` and the same linker script;
- no allocation across the boundary unless the allocator is explicitly passed;
- a safe Rust wrapper that documents the contract;
- feature-gated and covered by tests;
- reviewed by the same unsafe-code reviewer as Rust `unsafe`.

The preference is always to rewrite small C code in Rust rather than import a second toolchain.
The burden of proof is on adding C/C++, not on keeping it out.

## 7. Sources and verification

Fetched successfully:

| Ref | Source | Notes |
| --- | --- | --- |
| [1] | The rustc book, "`x86_64-unknown-none`": https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html | Tier 2, ELF, no std, alloc with allocator, no vector/FP by default, kernel code model, System V `extern "C"` without red zone |
| [2] | The rustc book, "Platform Support": https://doc.rust-lang.org/rustc/platform-support.html | `x86_64-unknown-uefi` Tier 2 without host tools, std marked "?" |
| [3] | The Embedded Rust Book, "A `no_std` Rust Environment": https://docs.rust-embedded.org/book/intro/no-std.html | `core` vs `std`, no runtime, alloc optional |
| [4] | The rustc book, "Codegen Options": https://doc.rust-lang.org/rustc/codegen-options/index.html | `panic=abort`, `linker-flavor=ld.lld`, `code-model`, `no-redzone`, `relocation-model` |
| [5] | The Rust Reference, "Inline assembly": https://doc.rust-lang.org/reference/inline-assembly.html | `asm!`, `naked_asm!`, `global_asm!`; stable on x86-64; operand restrictions |
| [6] | The Rust Reference, "Type layout": https://doc.rust-lang.org/reference/type-layout.html | `repr(Rust)` has no layout guarantees; `repr(C)` for FFI |
| [7] | The Rust Reference, "Behavior considered undefined": https://doc.rust-lang.org/reference/behavior-considered-undefined.html | UB list; `unsafe` does not change the rules |
| [8] | The Rustonomicon, "Meet Safe and Unsafe": https://doc.rust-lang.org/nomicon/meet-safe-and-unsafe.html | Safe/unsafe split; soundness responsibility |
| [9] | The `alloc` crate documentation: https://doc.rust-lang.org/alloc/ | Smart pointers, collections, global allocator interface |
| [10] | The Rust Programming Language, Appendix G, "How Rust is Made and Nightly Rust": https://doc.rust-lang.org/book/appendix-07-nightly-rust.html | Release train, stable default, nightly feature flags |
| [11] | The Rustonomicon, index: https://doc.rust-lang.org/nomicon/ | Warns the book is incomplete; Reference is preferred where they disagree |

Limitations:

- The Rustonomicon itself warns that it is incomplete and that the Reference should be assumed
  correct where the two disagree [11]. This note follows that rule: layout and UB claims come
  from the Reference, not from the Nomicon's prose.
- `core::arch` was not fetched page-by-page. The statement that not every instruction has a
  stable intrinsic wrapper is a general toolchain observation, and the stable fallback cited
  here is the inline-assembly Reference [5].
- No Aster code was compiled, so no claim is made about image size, compile time, or runtime
  performance.

Cross-links: [ADR 0003](../decisions/0003-rust-first-toolchain.md),
[Kernel architecture](kernel-architecture.md),
[UEFI boot](uefi-boot.md),
[x86_64 memory and interrupts](x86_64-memory-and-interrupts.md).
