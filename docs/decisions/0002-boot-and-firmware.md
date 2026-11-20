# ADR 0002: Boot and Firmware

Status: Accepted
Date: 2026-11-20

## Context

Aster OS targets x86_64 UEFI firmware. The boot path must:

- load a Rust `no_std` kernel from disk;
- obtain a linear framebuffer and a UEFI memory map;
- pass the ACPI RSDP and enough platform information to the kernel;
- enter the kernel in 64-bit mode with a known address-space layout;
- be testable in QEMU with OVMF before real hardware is available;
- avoid a dependency on a general-purpose boot loader or a legacy BIOS path.

The kernel is linked at `0xFFFFFFFF80000000` and expects the loader to provide a boot-info
structure. The address-space decision is in [ADR 0004](0004-address-space-layout.md); the
language and linker decision is in [ADR 0003](0003-rust-first-toolchain.md).

The UEFI specification is the contract. The target is UEFI 2.11 (released December 2024), with
2.10 as the compatibility baseline. The detailed research is in
[UEFI boot research](../research/uefi-boot.md).

## Decision

### Loader

Build **Aster Boot**, a custom UEFI application:

- Rust `no_std`, target `x86_64-unknown-uefi`, PE32+ EFI application;
- entry point `efi_main(EFI_HANDLE, EFI_SYSTEM_TABLE *)`;
- no UEFI shell, no boot menu, no general file-system stack;
- uses the firmware's `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` / `EFI_FILE_PROTOCOL` to read the kernel
  file from the ESP;
- locates GOP, selects a 32-bit-per-pixel linear-framebuffer mode, and records the mode;
- obtains the UEFI memory map, copies it into a loader-owned buffer, and records the map key;
- parses the kernel ELF64, copies `PT_LOAD` segments to their physical load addresses, zeroes
  the BSS (`p_memsz > p_filesz`), and records the virtual entry point;
- builds minimal page tables for the kernel image, the HHDM window, and the loader's own
  trampoline;
- calls `ExitBootServices()` with the current map key, loads the new `CR3`, sets `RSP` to a
  16-byte-aligned boot stack mapped in the new page tables, and jumps to the kernel;
- never returns to firmware and never calls a boot service after `ExitBootServices()`.

### Disk layout

- GPT partitioning, with a protective MBR and a backup GPT header.
- One EFI System Partition, FAT32, type GUID `C12A7328-F81F-11D2-BA4B-00A0C93EC93B`.
- Loader path: `\EFI\BOOT\BOOTX64.EFI` (removable-media fallback path).
- Kernel path: `\EFI\ASTER\ASTERKRN.ELF`.
- Optional build-info file: `\EFI\ASTER\BOOTINFO.TXT` (human-readable only; the kernel does not
  parse it).

The loader does not implement FAT32. It uses the firmware's FAT driver. This is a deliberate
reduction in pre-kernel code: the loader reads the kernel file and does not need directory-entry,
long-file-name, or cluster-chain logic. The optional `BOOTINFO.TXT` is for humans only.

### Boot-info ABI

The loader and kernel share a versioned, `#[repr(C)]` structure. Minimum fields:

| Field | Type | Purpose |
| --- | --- | --- |
| `magic` | `u64` | Identifies an Aster boot-info block |
| `version` | `u32` | ABI version; the kernel rejects unknown major versions |
| `size` | `u32` | Structure size for forward compatibility |
| `memory_map` | `PhysAddr` | Physical address of the copied UEFI memory map |
| `memory_map_entries` | `u64` | Number of descriptors |
| `memory_map_desc_size` | `u64` | Descriptor stride, not `sizeof` |
| `framebuffer_base` | `PhysAddr` | GOP linear framebuffer base |
| `framebuffer_size` | `u64` | Framebuffer size in bytes |
| `framebuffer_width` | `u32` | Visible width |
| `framebuffer_height` | `u32` | Visible height |
| `framebuffer_stride` | `u32` | Pixels per scan line |
| `framebuffer_format` | `u32` | RGB/BGR/bitmask enum |
| `framebuffer_masks` | `[u32; 4]` | Red, green, blue, reserved masks when bitmask |
| `acpi_rsdp` | `PhysAddr` | ACPI RSDP physical address, or 0 |
| `kernel_entry` | `VirtAddr` | Kernel virtual entry point |
| `kernel_phys_base` | `PhysAddr` | Physical load base of the kernel image |
| `kernel_size` | `u64` | Loaded kernel image size |
| `hhdm_base` | `VirtAddr` | Direct-map base, `0xFFFF800000000000` |
| `flags` | `u64` | Feature bits (framebuffer valid, ACPI valid, etc.) |

The loader passes the physical address of this structure in `RDI` at kernel entry. The kernel
entry stub converts it through the HHDM, reads it before enabling interrupts, and copies it into
kernel memory. The structure is independent of the loader. A future GRUB or Limine path could
fill the same structure for debugging; the kernel does not know which loader produced it.

### Firmware policy

- Target UEFI 2.11; use only interfaces present since UEFI 2.3 or earlier.
- Test with OVMF x64 under QEMU, q35 machine, single CPU, 512 MiB to 4 GiB RAM.
- Use `-pflash` for the firmware code and variable store when variable persistence or Secure
  Boot is being tested; `-bios` is acceptable for a quick smoke test.
- Keep Secure Boot disabled for bring-up. An unsigned loader is rejected when Secure Boot is
  enabled. Production signing or a signed shim is a separate project.
- Do not depend on UEFI runtime services in v1. If they are needed later, either identity-map
  them or call `SetVirtualAddressMap()` once and convert all pointers.
- Pass the ACPI RSDP to the kernel through the boot info; the kernel must not scan physical
  memory for ACPI after it owns the map.

### QEMU/OVMF test matrix

| Case | Firmware | RAM | Display | Purpose |
| --- | --- | --- | --- | --- |
| B1 | OVMF x64 DEBUG | 512 MiB | stdvga | Small memory map; low-memory assumptions |
| B2 | OVMF x64 DEBUG | 1024 MiB | stdvga | Default development |
| B3 | OVMF x64 DEBUG | 4096 MiB | stdvga | Large map; high memory and 64-bit BARs |
| B4 | OVMF x64 RELEASE | 1024 MiB | stdvga | No debug port; serial-only logging |
| B5 | OVMF x64 DEBUG | 1024 MiB | none | Headless CI; serial log |
| B6 | OVMF x64 + Secure Boot | 1024 MiB | stdvga | Rejection of unsigned loader; future signing |

The matrix is proposed, not executed. The exact OVMF file names and paths depend on the host
distribution and QEMU installation.

## Alternatives

### GRUB with Multiboot2

- Advantages: mature, handles many file systems and devices, menu and command line, widely
  tested.
- Disadvantages: another boot ABI; Multiboot2 typically hands off in 32-bit protected mode, so a
  64-bit kernel needs a trampoline; a larger pre-kernel code base and config surface; less
  control over the exact boot-info fields.
- Why rejected: the kernel would still need a translation layer from Multiboot2 to Aster's boot
  info, and the 32-bit handoff adds a stage that the native UEFI x64 path does not need.

### Limine

- Advantages: modern protocol designed for higher-half kernels; good QEMU support.
- Disadvantages: another protocol and dependency; still needs a translation layer to Aster's
  boot info; less ownership of the boot path.
- Why rejected: a reasonable fallback, but not the primary path.

### UEFI stub in the kernel

- Advantages: one less stage; firmware loads the kernel directly.
- Disadvantages: the kernel contains firmware-facing code and PE/COFF packaging; UEFI protocol
  handling leaks into the kernel; non-UEFI boot paths become harder.
- Why rejected: separation of concerns. The loader is small and testable; the kernel stays a
  freestanding ELF image.

### BIOS/MBR

- Advantages: works on older machines; simpler firmware interface in some ways.
- Disadvantages: no GOP linear framebuffer standard, no native 64-bit handoff, more assembly,
  and a legacy platform that is irrelevant to the product target.
- Why rejected: the target is UEFI x86_64.

### coreboot or a UEFI payload

- Advantages: open firmware, fast boot, control over the platform.
- Disadvantages: a separate firmware project with its own hardware support matrix; far outside
  the first milestone.
- Why rejected: not needed to boot Aster on UEFI machines or in QEMU.

## Consequences

Positive:

- Full control over the boot-info ABI and the handoff state.
- Native 64-bit UEFI entry, no 32-bit trampoline.
- A small pre-kernel code base that can be audited and tested.
- The loader can pass exactly the framebuffer, memory map, and ACPI information the kernel
  needs.
- The kernel remains a normal ELF image, which is easier to debug than a PE image.

Negative:

- The project owns ELF loading, GOP mode selection, memory-map handling, page-table setup, and
  firmware error paths.
- Firmware quirks will require real-hardware testing and a debug log path.
- Secure Boot requires signing or a shim, which is extra work.
- The loader must be updated if the boot-info ABI changes; versioning is mandatory.
- The loader only reads the FAT32 ESP, by design.

Neutral:

- GPT and FAT32 are UEFI conventions, not Aster inventions. The kernel's own file system is a
  separate project.
- Using the firmware's FAT driver means the loader trusts firmware for file reads. That is
  acceptable for boot; the kernel verifies what it can after handoff.

## Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Memory map changes between `GetMemoryMap` and `ExitBootServices` | High | Boot failure | No allocations in the window; bounded retry; log the map key |
| GOP exposes only `PixelBltOnly` or an unsupported format | Medium | No display | Enumerate modes; fail with a serial diagnostic; test multiple OVMF builds |
| Framebuffer stride differs from width | Medium | Skewed output | Store and use `PixelsPerScanLine` |
| Firmware bug or real-hardware difference | Medium | Boot failure | OVMF matrix first; one real UEFI machine before declaring stable; serial log |
| Secure Boot rejects the unsigned loader | High when enabled | Boot failure | Keep Secure Boot off for bring-up; plan signing/shim |
| Runtime-services mapping fault | Medium | Crash after boot | Do not use runtime services in v1; use `SetVirtualAddressMap` or identity map later |
| Boot-info ABI drift | Medium | Kernel/loader mismatch | Magic, version, size, and a compatibility check; test both binaries together |
| Kernel image grows beyond the loader's assumptions | Low | Load failure | Check ELF segment bounds; reserve a defined kernel window; fail early |

## Follow-ups

1. Write the boot-info ABI definition in `shared/` with `#[repr(C)]` types and a version
   constant. Do not duplicate the struct in the loader and kernel.
2. Implement Aster Boot in `boot/` with `x86_64-unknown-uefi`, using the firmware Simple File
   System protocol.
3. Add a GPT/FAT32 image builder in `tools/` that produces a reproducible `aster.img`.
4. Add an OVMF smoke test to CI: build loader and kernel, boot B2, check a serial marker.
5. Add the B1/B3/B4 cases and record the memory map and framebuffer mode in the log.
6. Test one real UEFI machine before calling the boot path stable.
7. Design the Secure Boot path: sign the loader with an enrolled key, or ship a signed shim.
8. Decide the runtime-services policy after the first boot; default remains "not used".

## Related documents

- [UEFI boot research](../research/uefi-boot.md)
- [ADR 0001: Kernel architecture](0001-kernel-architecture.md)
- [ADR 0003: Rust-first toolchain](0003-rust-first-toolchain.md)
- [ADR 0004: Address space layout](0004-address-space-layout.md)
