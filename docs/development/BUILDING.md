# Building Aster OS

Aster OS builds on Windows, Linux, and macOS hosts. The host is only a build
and test environment; it is not a runtime dependency of the OS.

## 1. Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust (rustup) | pinned by `rust-toolchain.toml` (1.100.0) | kernel, bootloader, desktop |
| Targets | `x86_64-unknown-none`, `x86_64-unknown-uefi` | freestanding kernel/desktop, UEFI app |
| QEMU | 8.0+ | boot testing |
| OVMF | any recent EDK2 build | UEFI firmware for QEMU |
| Python | 3.10+ | build/image/test tooling |
| Pillow | optional | regenerate the font atlas only |

Clang, NASM, CMake, and a C toolchain are **not** required. Linking uses
`rust-lld`; assembly is emitted through Rust's `global_asm!` and LLVM's
integrated assembler.

### Install Rust

```bash
# Windows: winget install Rustlang.Rustup
# macOS:   brew install rustup-init && rustup-init
# Linux:   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-unknown-none x86_64-unknown-uefi
```

`rust-toolchain.toml` pins the channel and targets. If a standalone system Rust
is also installed, the build scripts prefer `~/.cargo/bin/cargo` so the rustup
toolchain and targets are used.

### Install QEMU

```bash
# Windows
winget install SoftwareFreedomConservancy.QEMU
# macOS
brew install qemu
# Debian/Ubuntu
sudo apt install qemu-system-x86
```

### Firmware

```bash
python tools/fetch_firmware.py
```

This downloads the `rust-osdev/ovmf-prebuilt` EDK2 build (BSD-2-Clause-Patent)
into `tools/firmware/`, which is git-ignored. `tools/run.py` combines
`vars.fd` + `code.fd` into a 4 MiB pflash image at `build/OVMF.fd`.

If you already have OVMF, set `OVMF_CODE` to a 4 MiB pflash image.

### Fonts (only when changing the atlas)

```bash
python -m pip install pillow
python tools/fontgen.py
```

This downloads Inter and JetBrains Mono (both OFL) into `assets/fonts/`
(git-ignored), rasterizes the faces, and regenerates
`assets/fonts/atlas.bin` plus `user/asterdesk/src/font_data.rs`. The generated
atlas is committed, so a normal build does not need Pillow or network access.

## 2. Build

```bash
python tools/build.py            # full build
python tools/build.py --clean    # remove target/ and rebuild
python tools/build.py --boot-only
```

Outputs:

```
build/asterkernel.elf   higher-half ELF kernel
build/asterdesk.elf     ring-3 desktop ELF
build/aster.img         512 MiB GPT disk with one FAT32 ESP
build/OVMF.fd           combined UEFI firmware for QEMU
```

The ESP contains:

```
/EFI/BOOT/BOOTX64.EFI   Aster Boot
/KERNEL.ELF             kernel
/DESK.ELF               desktop + apps
```

The image builder deliberately uses 8.3 names (`KERNEL.ELF`, `DESK.ELF`) so the
boot path does not depend on long-file-name parsing. The kernel's ramfs
supports long names; this restriction applies only to the ESP.

The image is verified after every build: GPT header CRC, partition bounds, FAT
cluster chains, directory traversal, and byte-for-byte file contents. A build
with a corrupt image fails.

## 3. Run

```bash
python tools/run.py                          # interactive QEMU window + serial
python tools/run.py --headless --timeout 12 --screenshot build/screen.ppm
python tools/run.py --headless --serial file:build/serial.log
python tools/run.py --debug                  # -S -s gdb stub on :1234
```

Reference machine: q35, 1 vCPU, 512 MiB RAM, std VGA (GOP), PS/2 keyboard and
mouse, IDE disk, `-display none` or a GUI window. Serial is COM1.

QEMU flags of note:

- `-drive if=pflash,format=raw,readonly=on,file=build/OVMF.fd` (a 4 MiB flash
  image; do not use `-bios` with the split 3.6 MB code file).
- Do **not** add `-device ps2-mouse` on q35: the PS/2 mouse is part of the
  default chipset and the device name is rejected.
- `-rtc base=localtime` so the Calendar and top bar match the host time.

## 4. Tests

```bash
python -m unittest discover -s tests -v   # host image tests
python tools/test.py                     # build + boot smoke
python tools/test.py --ui                # + scripted UI interaction
python tools/bench.py                    # boot/frame measurements
```

See [TESTING.md](TESTING.md).

## 5. Repository conventions

- Every kernel/user ABI change must bump the version in `shared/aster-abi`.
- Generated files (`font_data.rs`, `atlas.bin`) are committed; the source fonts
  are not.
- `build/` and `target/` are disposable.
- The bootloader requires page-aligned ELF segments (the linker scripts
  guarantee this).

## 6. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `could not load PC BIOS` | QEMU `-bios` cannot map the 3.6 MB OVMF code file. Use the combined 4 MiB pflash image produced by `run.py` / `fetch_firmware.py`. |
| `Parameter 'driver' expects a pluggable device type` | Remove `-device ps2-mouse`; q35 already provides one. |
| No bootable device / `Not Found` | The ESP or GPT is invalid. Run `python tools/build.py` and check the verifier output. |
| Black screenshot | The desktop was not yet ready. Wait for `DESKTOP-READY` or increase `--timeout`; the first frame appears about 4.3 s after QEMU start on TCG. |
| `can't find crate for core` / target not installed | `rustup target add x86_64-unknown-none x86_64-unknown-uefi`; ensure you are using rustup's cargo. |
| Desktop missing but kernel boots | `DESK.ELF` missing from the ESP or ELF rejected. Check the serial log (`[err] open ...`, `init: cannot start the desktop`). |
| Image verification fails | `tools/image.py` prints the failing file or CRC. Re-run the build; never bypass the verifier. |
| Mouse does not move in a headless harness | QMP absolute pointer events are not translated to PS/2 relative motion. Use relative events (`tools/pointer_test.py`) or a real mouse. |

## 7. CI

`.github/workflows/ci.yml` installs the pinned toolchain, fetches OVMF, builds,
runs host tests, and boots the image headlessly with QEMU. The UI script is
optional because it needs VNC/QMP and longer timeouts. CI uses TCG, not KVM, so
timings there are slower than the reference numbers.

## 8. Reproducibility

- The toolchain is pinned in `rust-toolchain.toml`.
- The OVMF build is pinned by release tag in `tools/fetch_firmware.py`.
- The image builder uses fixed GUIDs, fixed FAT timestamps, and deterministic
  directory ordering.
- Rust builds include debug info; binary equality across toolchain patch
  releases is not guaranteed. The *image layout* is reproducible.
