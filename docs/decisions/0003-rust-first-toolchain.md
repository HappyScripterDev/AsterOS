# ADR 0003: Rust-First Toolchain

Status: Accepted
Date: 2026-11-20

## Context

Aster OS needs a toolchain that:

- produces freestanding x86_64 kernel and UEFI loader binaries;
- supports hand-written entry code, GDT/IDT loads, ISR stubs, and context switching;
- does not require a C/C++ runtime, libc, clang, or NASM;
- is reproducible and stable enough that a compiler upgrade does not silently change the
  kernel's behavior;
- gives the small team memory-safety and ownership benefits without hiding the hardware.

Rust is the chosen implementation language. The language research is in
[Rust in the kernel](../research/rust-in-the-kernel.md). The kernel architecture is in
[ADR 0001](0001-kernel-architecture.md); the boot design is in
[ADR 0002](0002-boot-and-firmware.md).

## Decision

### Language and runtime

- Rust **stable**, pinned to an exact version in `rust-toolchain.toml` (a follow-up file).
- Kernel: `#![no_std]` plus `core`; `alloc` only after the kernel heap and global allocator are
  initialized.
- Loader: `#![no_std]` plus `core`; target `x86_64-unknown-uefi`.
- `panic = "abort"` for both. Panics are fatal and log to serial before halting.
- No external crates in the kernel without a review of `unsafe`, licensing, `no_std` support,
  and maintenance. The default dependency count is zero.

### Assembly

Assembly is allowed only for:

- `_start` / trampoline entry;
- `LGDT`, `LIDT`, `LTR`;
- ISR stubs for vectors with and without error codes;
- context switch;
- instructions without a stable Rust intrinsic (`INVLPG`, `SWAPGS`, `STAC`/`CLAC`, `HLT`).

Use `global_asm!` for whole functions, `naked_asm!` for naked entry stubs, and `asm!` for
inline instructions. The Rust Reference documents all three as stable on x86-64 [1]. No NASM
files and no separate assembler pass.

### Linking

- Use `rust-lld` only.
- `-C linker-flavor=ld.lld`; point `-C linker` at `rust-lld` if it is not on `PATH`.
- `-C relocation-model=static`, `-C code-model=kernel`, `-C panic=abort`.
- A project linker script places the kernel at `0xFFFFFFFF80000000` with a physical LMA that the
  loader reads from the ELF program headers.
- `-C opt-level=s` or `z`, LTO, and `codegen-units=1` for size and determinism.
- The UEFI loader is a PE32+ image produced for `x86_64-unknown-uefi`; the kernel is ELF64 and
  is parsed by the loader, so no `objcopy` step is required.

### Unsafe policy

The full rule set is in [Rust in the kernel](../research/rust-in-the-kernel.md), Section 4.
The enforceable summary:

- every `unsafe` block has a `// SAFETY:` comment with the invariant and justification;
- `unsafe` is confined to `arch`, `mm::paging`, `mm::frame`, `sync`, `hal`, and syscall entry;
- `#![deny(unsafe_op_in_unsafe_fn)]` and `#![deny(static_mut_refs)]`;
- no `static mut`; use `UnsafeCell` plus a lock or atomics;
- no references to MMIO; use raw pointers and volatile operations;
- user pointers are validated and copied through fallible helpers;
- ABI boundaries use `#[repr(C)]` or `#[repr(transparent)]`;
- new unsafe code is reviewed by a second person.

### C/C++ boundary

C or C++ is not used for the kernel or loader in v1. It would be considered later only for:

1. vendor driver code with no reasonable Rust equivalent;
2. certified cryptographic code tied to a C implementation;
3. a performance-critical routine with an existing, well-tested C implementation;
4. UEFI reference code used for validation, not shipped.

Any C/C++ component must be isolated behind a C ABI, compiled freestanding with no libc, no
exceptions, and no unwinding, linked with the same `rust-lld` and linker script, wrapped in a
safe Rust API, feature-gated, and reviewed like unsafe Rust. The burden of proof is on adding
the second toolchain.

## Alternatives

### C or C++ first

- Advantages: mature compilers, debuggers, and firmware examples; direct access to existing
  headers; predictable code generation.
- Disadvantages: memory-safety bugs are the default failure mode; the project would either
  rewrite in Rust later or carry a large unsafe C base forever; C++ exceptions and RTTI add
  runtime machinery that a kernel does not want.
- Why rejected: the project's differentiator is a new, memory-safe kernel. Starting in C would
  spend the safety budget before the architecture exists.

### Rust nightly

- Advantages: access to unstable features, `build-std`, newer target support, experimental
  intrinsics.
- Disadvantages: features can change or be removed; reproducibility is harder; a compiler
  upgrade can break the build for reasons unrelated to Aster.
- Why rejected: the stable target set is sufficient for the first milestone. Nightly is allowed
  for experiments, not for the shipping kernel.

### Rust plus clang and NASM

- Advantages: familiar C/assembly tooling; easier to reuse existing assembly snippets; Clang
  can compile C helpers.
- Disadvantages: two or three toolchains to install and version; more places for ABI and
  linker-script mismatches; NASM syntax differs from the LLVM integrated assembler.
- Why rejected: `global_asm!`/`asm!` and `rust-lld` cover the required assembly and linking.
  Adding clang/NASM increases the dependency surface without solving a problem the project has.

### Rust plus a C compiler for third-party code from day one

- Advantages: can reuse existing drivers and libraries immediately.
- Disadvantages: two build systems, two ABIs, two debugging models, and a larger trusted base
  before the kernel boots.
- Why rejected: no third-party driver is required for the first milestone. Add C only when a
  specific component justifies it.

### Zig or another systems language

- Advantages: modern safety features, C interop, simple build.
- Disadvantages: a smaller OS-development ecosystem, fewer bare-metal target guarantees, and a
  less mature story for the specific x86_64 paging/interrupt code Aster needs.
- Why rejected: Rust's `no_std` ecosystem, inline assembly, and existing OS projects provide a
  stronger starting point for this team.

## Consequences

Positive:

- Memory safety and ownership in the kernel, with a small, auditable unsafe surface.
- One compiler, one linker, one assembly syntax, and one build system.
- No libc, no C runtime, no C++ exception machinery.
- `core` and `alloc` provide the data structures a kernel needs without a hosted runtime.
- The stable channel and pinned version make builds reproducible.
- Inline assembly and `rust-lld` are sufficient for entry, paging, interrupts, and linking.

Negative:

- `x86_64-unknown-none` is Tier 2, not Tier 1; the Rust project does not guarantee the same
  level of testing as a Tier 1 target [2].
- Code size can grow through monomorphization; the kernel needs size budgets and LTO.
- Compile times can be longer than C for small changes, especially with LTO.
- Not every instruction has a stable `core::arch` intrinsic; inline assembly is the fallback.
- Rust's ABI is not stable; every boundary needs `repr(C)` and careful review.
- The `no_std` ecosystem is smaller; the project will implement more infrastructure itself.
- Debugging a Rust kernel is less common than debugging a C kernel; tooling and documentation
  are thinner.

Neutral:

- "No clang" means no clang driver, not no LLVM. `rust-lld` and the LLVM integrated assembler
  are still LLVM components.
- Rust does not eliminate the need for hardware documentation, timeouts, or tests. It changes
  which bugs are likely, not whether bugs exist.

## Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Tier 2 target regression | Medium | Build or boot failure | Pin the toolchain; boot-test every upgrade; keep a known-good toolchain |
| Code size exceeds the kernel window or cache budget | Medium | Boot or performance failure | `opt-level=s/z`, LTO, `codegen-units=1`, size reporting in CI |
| Unsafe code is unsound | Medium | Memory corruption | Confine unsafe; SAFETY comments; second review; host tests for pure logic; fuzzing |
| Inline assembly constraints are wrong | Medium | Corruption or crashes | Small stubs; explicit clobbers; `options` only when true; review and disassemble |
| ABI mismatch at syscall/boot/UEFI boundary | Medium | Crash or data corruption | `repr(C)`; shared ABI crate; compile-time size/offset assertions |
| Compiler or LLVM codegen bug | Low | Subtle miscompile | Pin; test; keep a minimal reproducer; avoid exotic features |
| Missing intrinsic forces assembly | Medium | Maintenance burden | Keep assembly isolated in `arch`; document each instruction |
| Toolchain install is not reproducible | Low | CI drift | Pin exact version; cache the toolchain; document `rustup target add` |

## Follow-ups

1. Create `rust-toolchain.toml` with the exact stable version and the two targets.
2. Add CI jobs for `cargo build`, `clippy -D warnings`, `rustfmt --check`, and the OVMF boot
   smoke test.
3. Add a size report for the kernel and loader to every CI run.
4. Add a `// SAFETY:` lint or review checklist and enforce it in code review.
5. Add QEMU gdb stub support and a symbol-loading script for kernel debugging.
6. Document the linker script, section layout, and the kernel's physical/virtual load addresses.
7. Re-evaluate the C/C++ boundary only when a concrete component justifies it; record that
   decision in a new ADR.

## Related documents

- [Rust in the kernel](../research/rust-in-the-kernel.md)
- [ADR 0001: Kernel architecture](0001-kernel-architecture.md)
- [ADR 0002: Boot and firmware](0002-boot-and-firmware.md)
- [ADR 0004: Address space layout](0004-address-space-layout.md)
