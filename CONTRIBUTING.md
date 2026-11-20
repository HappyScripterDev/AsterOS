# Contributing to Aster OS

Aster OS is an operating system, not an application. Changes must be correct,
testable, and documented before they are merged.

## Ground rules

1. **No Linux, Windows, or macOS runtime code.** Host tools may build and test
   the system; they must never become runtime dependencies.
2. **Never fake functionality.** If a control, driver, syscall, or app is not
   implemented, it must be absent or clearly report the limitation. Do not add
   buttons that only look functional.
3. **Build stays green.** Every change must pass `python tools/test.py`.
4. **Architecture changes need an ADR.** Add `docs/decisions/NNNN-title.md`
   before changing a shared interface (boot ABI, syscall ABI, memory layout,
   filesystem contract, scheduler policy).
5. **No silent warnings.** Compiler warnings may be allowed temporarily, but
   new warnings need a justification in the pull request.
6. **Unsafe Rust requires a safety comment** explaining the invariants the
   caller must uphold. Unsafe blocks in the kernel are not stylistic.
7. **No new dependency without a license check** and an entry in
   `THIRD_PARTY.md`.

## Code layout

- `boot/`: UEFI loader. No kernel dependencies. Keep the UEFI bindings isolated
  in `boot/src/efi.rs`.
- `kernel/`: everything privileged. Subsystems live in `arch/`, `mm/`, `fs.rs`,
  `sched.rs`, `syscall.rs`, `drivers/`. The kernel is `no_std` + `alloc`.
- `shared/aster-abi`: the boot and syscall contract. Changes require a version
  bump and a coordinated update of bootloader, kernel, and desktop.
- `user/asterdesk`: ring 3. It may only use the syscall ABI; never hardware.
- `tools/`: reproducible build/test tooling. Python 3.10+, stdlib only except
  Pillow for `fontgen.py`.

## Style

- Rust 2021, `rustfmt` defaults. C/C++ (future) uses `clang-format`.
- Names describe purpose; avoid abbreviations except established ones (PMM,
  VMM, IDT, GDT, APIC).
- Kernel logging uses `info!`, `warn!`, `error!`, `debug!` with subsystem
  prefixes (`pmm:`, `vfs:`, `sched:` ...).
- Comments explain *why*, not *what*. Public unsafe functions document their
  preconditions.

## Workflow

1. Open an issue/ADR describing the problem and the interface impact.
2. Implement in small, buildable commits.
3. Add or update tests:
   - host-side tests in `tests/` for pure logic and image tooling,
   - kernel self-tests for memory/filesystem invariants,
   - a QEMU scenario for drivers or UI when applicable.
4. Update `docs/PROJECT_STATUS.md` and, if user-visible, `docs/FEATURES.md`.
5. Record measured results if the change claims a performance improvement.

## Reporting bugs

Include the serial log (`build/serial.log`), the QEMU command line, the build
commit, and the exact steps. For UI bugs include a screenshot and the pointer
position if relevant. Never paste secrets or personal data.

## Commit messages

```
subsystem: short imperative summary

Why: one or two sentences on the problem.
Tested: build, boot, tests run, screenshots.
```

## Review checklist

- [ ] Builds with `python tools/build.py` from a clean checkout.
- [ ] `python tools/test.py` passes.
- [ ] No new fake UI or unimplemented claim.
- [ ] Syscall/boot ABI unchanged, or versioned and documented.
- [ ] Security impact considered (pointer validation, privileges, W^X).
- [ ] Documentation updated.
