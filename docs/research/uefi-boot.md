# UEFI Boot Research

Status: Research note supporting [ADR 0002](../decisions/0002-boot-and-firmware.md)
Date: 2026-11-20
Scope: UEFI boot flow, GOP, memory map, ExitBootServices, custom loader versus GRUB, GPT/FAT32
ESP, and OVMF testing.

Aster OS has not been booted. The command lines and test matrices below are proposed, not
executed.

## 0. Terminology correction

`ExitBootServices()` removes **boot services**, not runtime services. After it returns, the
loader and kernel may no longer call boot services (console I/O, file I/O, memory allocation,
protocol lookup, `Blt`, `Stall`, and so on). Runtime services (`GetTime`, `SetVariable`,
`ResetSystem`, and others) remain available, but only under the mapping rules in Section 6.
This distinction matters: a loader that expects runtime services to disappear will
unnecessarily duplicate firmware functionality, while a loader that expects boot services to
survive will crash on the first `ConOut->OutputString` after exit.

## 1. Target firmware and specification version

- Target: x86_64 UEFI firmware (no legacy BIOS requirement).
- Specification: UEFI Specification 2.11, released December 2024, which is the current version
  listed by the UEFI Forum as of this note [1].
- Previous baseline: UEFI 2.10 (August 2022) and 2.10 Errata A (August 2024) [1].
- Aster Boot will use only interfaces that have existed since UEFI 2.3 or earlier, so it should
  run on 2.10 and on most 2.8+ firmware. The build and test target is 2.11.
- Platform initialization (SEC/PEI/DXE/BDS) is defined by the UEFI Platform Initialization
  specification, not by the UEFI specification itself. The current PI specification listed by
  the Forum is 1.10 (July 2026) [1]. Aster treats those phases as firmware-internal.

Why this matters: the UEFI specification is the contract for image format, services, memory map,
and boot manager behavior. The PI specification explains how firmware gets to the point where
that contract exists. Mixing the two leads to incorrect assumptions about what the OS can call.

## 2. UEFI boot flow

### 2.1 Firmware side

1. The platform firmware initializes the CPU, chipset, and memory, then enters the UEFI boot
   manager (the PI phases are firmware-internal).
2. The boot manager selects a boot option. For an installed OS this is normally a `Boot####`
   variable containing a device path and load option. For removable media, the specification
   defines a fallback path: `\EFI\BOOT\BOOTX64.EFI` on the EFI System Partition (UEFI 2.11,
   Section 3.5.1.1, "Removable Media Boot Behavior") [2].
3. Firmware loads the selected image as a PE32+ executable. On x64, the entry point has the
   signature `EFI_STATUS efi_main(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable)`
   (UEFI 2.11, Sections 2.1.1 and 2.3.4) [2].
4. The x64 UEFI calling convention is the Microsoft x64 convention: first four arguments in
   `RCX`, `RDX`, `R8`, `R9`, the rest on the stack, 32 bytes of shadow space, and `EFI_STATUS`
   returned in `RAX` (UEFI 2.11, Section 2.3.4) [2].
5. `SystemTable->BootServices`, `SystemTable->RuntimeServices`, and
   `SystemTable->ConfigurationTable` are the loader's entry points into firmware (UEFI 2.11,
   Section 4.3) [2]. The configuration table carries the ACPI RSDP and other standard tables
   (Section 4.6).

### 2.2 Aster Boot side

1. Disable the firmware watchdog with `SetWatchdogTimer(0)` so a slow boot is not reset, then
   locate the Graphics Output Protocol (GOP) and select a mode (Section 3).
2. Use `EFI_LOADED_IMAGE_PROTOCOL` on `ImageHandle` to obtain the device handle, then open
   `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` on that device and read the kernel file from the ESP using
   `EFI_FILE_PROTOCOL` (UEFI 2.11, Chapters 9 and 13) [2]. This deliberately uses the firmware's
   FAT driver instead of embedding a FAT32 driver in the loader. The kernel gets its own file
   system later; the loader only needs to read the kernel file.
3. Parse the kernel ELF64, copy `PT_LOAD` segments to their physical load addresses, zero the
   difference between `p_memsz` and `p_filesz` (BSS), and record the entry point and boot
   information.
4. Allocate and zero the boot-info structure, copy the UEFI memory map into it, and record the
   framebuffer and ACPI RSDP.
5. Build a minimal set of page tables: identity map the loader's current code/data, map the
   kernel image at its higher-half virtual address, and map the HHDM window (see
   [ADR 0004](../decisions/0004-address-space-layout.md)). The page-table format is the 4-level
   paging format in Intel SDM Vol. 3A, Chapter 5, "Paging" [9].
6. Call `ExitBootServices()` with the current map key (Section 5).
7. Load the new `CR3`, reload the data segments, set `RSP` to a 16-byte-aligned boot stack that
   is mapped in the new page tables, put the boot-info physical address in `RDI`, and jump to the
   kernel entry point. There is no return to firmware.

The loader must not call any boot service after step 6. In particular, it must not print to the
UEFI console, read a file, allocate memory, or call GOP `Blt` after that point.

### 2.3 Image format and build target

- Aster Boot is a PE32+ EFI application. The Rust target `x86_64-unknown-uefi` is listed as a
  Tier 2 target without host tools, with standard-library support marked as work in progress
  [8]. Building it with `#![no_std]` and `core` avoids that dependency.
- The kernel is an ELF64 `x86_64-unknown-none` binary, not a PE image. The loader owns the
  ELF-to-physical-memory step, which is one reason a custom loader is useful. The kernel linker
  script must set the physical load address (LMA) separately from the higher-half virtual
  address (VMA); otherwise the loader has no RAM address to copy the segments to.
- The loader and kernel share a versioned boot-info structure. The structure must be
  `#[repr(C)]`, fixed-width, and independent of Rust's default layout (see
  [Rust in the kernel](rust-in-the-kernel.md)).

## 3. Graphics Output Protocol (GOP)

GOP is defined in UEFI 2.11, Chapter 12, "Protocols - Console Support" [2]. The loader uses:

- `EFI_GRAPHICS_OUTPUT_PROTOCOL.QueryMode()` to enumerate modes;
- `EFI_GRAPHICS_OUTPUT_PROTOCOL.SetMode()` to select one;
- `EFI_GRAPHICS_OUTPUT_PROTOCOL.Mode` for the current mode.

The mode structure contains:

| Field | Meaning | Aster use |
| --- | --- | --- |
| `MaxMode` | Number of modes | Iteration bound for `QueryMode` |
| `Mode` | Current mode number | Diagnostic and boot-info record |
| `Info` | Pointer to `EFI_GRAPHICS_OUTPUT_MODE_INFORMATION` | Resolution, format, stride |
| `SizeOfInfo` | Size of `Info` | Validation |
| `FrameBufferBase` | Physical base address of the linear framebuffer | Boot-info field |
| `FrameBufferSize` | Size in bytes | Boot-info field and mapping size |

`Info` contains `HorizontalResolution`, `VerticalResolution`, `PixelFormat`,
`PixelInformation` (masks for `PixelBitMask`), and `PixelsPerScanLine`.

Pixel formats:

- `PixelRedGreenBlueReserved8BitPerColor` (32 bits per pixel, RGBX);
- `PixelBlueGreenRedReserved8BitPerColor` (32 bits per pixel, BGRX);
- `PixelBitMask` (use the four masks);
- `PixelBltOnly` (no linear framebuffer; only `Blt` is available).

Design rules:

1. Prefer a 32-bit-per-pixel RGB or BGR mode at the highest resolution that fits the intended
   memory budget. A 1920x1080x4 framebuffer is about 7.9 MiB (8.3 MB); a 3840x2160x4 framebuffer
   is about 31.6 MiB (33.2 MB). These are arithmetic examples, not measurements.
2. Reject `PixelBltOnly` for the first milestone. If no linear framebuffer mode is available,
   print a diagnostic and halt. Supporting `Blt` would require a shadow framebuffer and a
   firmware blit path that disappears at `ExitBootServices`.
3. Record `PixelsPerScanLine`, not just `HorizontalResolution`. The framebuffer stride can be
   larger than the visible width; using the width as the stride produces skewed output.
4. Record the pixel format and masks in the boot info. The kernel and the desktop compositor
   must not guess.
5. Do not assume the framebuffer is write-combining or cached. The kernel maps it with the
   appropriate memory type (write-combining if PAT is configured, otherwise uncached) and the
   compositor respects that mapping.

The framebuffer is plain physical memory after `ExitBootServices`. The protocol function
`Blt` is a boot service and is no longer callable. The loader passes the base, size, stride, and
format to the kernel; the kernel later maps the framebuffer into the desktop process through a
syscall.

## 4. UEFI memory map

`GetMemoryMap()` is UEFI 2.11, Section 7.2.3 [2]. Its signature is:

```
EFI_STATUS GetMemoryMap(
    UINTN *MemoryMapSize,
    EFI_MEMORY_DESCRIPTOR *MemoryMap,
    UINTN *MapKey,
    UINTN *DescriptorSize,
    UINT32 *DescriptorVersion);
```

The descriptor is:

```
typedef struct {
    UINT32 Type;
    EFI_PHYSICAL_ADDRESS PhysicalStart;
    EFI_VIRTUAL_ADDRESS VirtualStart;
    UINT64 NumberOfPages;
    UINT64 Attribute;
} EFI_MEMORY_DESCRIPTOR;
```

Practical rules:

1. Call once with `*MemoryMapSize = 0` to obtain the required size, then allocate a buffer with
   extra slack. The map can grow between calls; a buffer sized exactly to the first answer is a
   common source of `EFI_BUFFER_TOO_SMALL`.
2. `DescriptorSize` is not necessarily `sizeof(EFI_MEMORY_DESCRIPTOR)`; iterate with the
   returned stride.
3. Keep the `MapKey` from the most recent successful call. Do not allocate, load files, or call
   protocols that can change the map between that call and `ExitBootServices`.
4. `Attribute` bits matter. `EFI_MEMORY_RUNTIME` marks runtime-services regions;
   `EFI_MEMORY_XP` requests no-execute; `EFI_MEMORY_WB`, `WT`, `UC`, and `WC` describe cache
   behavior. The kernel should translate these into page-table permissions and memory types,
   not ignore them.
5. Check `DescriptorVersion`. The standard `EFI_MEMORY_DESCRIPTOR` version is 1; an unknown
   version should be rejected or handled explicitly, not parsed as if it were version 1.

Memory types and intended handling:

| UEFI type | Value | Aster handling |
| --- | --- | --- |
| `EfiReservedMemoryType` | 0 | Never use |
| `EfiLoaderCode` | 1 | Reclaim after the kernel has its own image and boot info copied |
| `EfiLoaderData` | 2 | Reclaim after boot info is copied; may contain the boot-info buffer |
| `EfiBootServicesCode` | 3 | Reclaim after `ExitBootServices` |
| `EfiBootServicesData` | 4 | Reclaim after `ExitBootServices` |
| `EfiRuntimeServicesCode` | 5 | Preserve; map according to runtime-services policy |
| `EfiRuntimeServicesData` | 6 | Preserve; map according to runtime-services policy |
| `EfiConventionalMemory` | 7 | Free for the frame allocator |
| `EfiUnusableMemory` | 8 | Never use |
| `EfiACPIReclaimMemory` | 9 | Preserve until ACPI tables are parsed, then reclaim |
| `EfiACPIMemoryNVS` | 10 | Preserve; firmware may use it across sleep |
| `EfiMemoryMappedIO` | 11 | Map only as MMIO, uncached, NX |
| `EfiMemoryMappedIOPortSpace` | 12 | Not RAM; do not add to the frame allocator |
| `EfiPalCode` | 13 | Preserve if present; not expected on x64 |
| `EfiPersistentMemory` | 14 | Treat as persistent memory, not general RAM |
| `EfiUnacceptedMemoryType` | 15 | Not expected on this platform; do not use blindly |

The loader should copy the map into a buffer that the kernel can read after the jump. A simple
approach is to allocate one page with `AllocatePages(AllocateAnyPages, EfiLoaderData, ...)`,
copy the descriptors there, and pass the physical address and count in the boot info. The kernel
copies the map into its own heap early, before it reuses loader memory.

## 5. ExitBootServices

`ExitBootServices()` is UEFI 2.11, Section 7.4.6 [2]. Its signature is:

```
EFI_STATUS ExitBootServices(
    EFI_HANDLE ImageHandle,
    UINTN MapKey);
```

Rules and consequences:

1. `MapKey` must come from the most recent `GetMemoryMap()`. If the map changed, firmware may
   return `EFI_INVALID_PARAMETER`. The loader must reacquire the map and retry. A bounded retry
   loop (for example, 3 attempts) is enough; an unbounded loop hides a real bug.
2. On success, boot services are terminated. The loader may not call `AllocatePool`,
   `AllocatePages`, `FreePool`, `FreePages`, `HandleProtocol`, `LocateProtocol`, `OpenProtocol`,
   `CloseProtocol`, `LoadImage`, `StartImage`, `Stall`, `SetTimer`, `ConOut`, `ConIn`, GOP
   `Blt`, or any other boot service.
3. Boot-services code and data become reclaimable. The loader's own code and data are still
   mapped until the kernel replaces the page tables; the kernel must not overwrite the loader
   before it has copied the boot info and left the trampoline.
4. Runtime-services code and data remain. They are marked `EFI_MEMORY_RUNTIME` and must not be
   reclaimed.
5. The loader must have its own stack, GDT, IDT, and page tables ready. Firmware interrupt state
   is not a stable contract; mask interrupts, install the kernel's IDT, reprogram the PIC/APIC,
   and only then enable interrupts.
6. The loader must not return to firmware. It jumps to the kernel entry point with a known
   register state and a known stack.

A safe sequence is:

```
GetMemoryMap -> allocate boot-info buffer -> copy map -> build page tables
-> disable interrupts -> GetMemoryMap again (confirm key) -> ExitBootServices
-> on failure, retry from GetMemoryMap -> on success, load CR3 -> jump
```

The exact ordering of page-table construction and `ExitBootServices` is flexible, but no
allocation may occur between the final `GetMemoryMap` and `ExitBootServices`.

## 6. Runtime services after ExitBootServices

Runtime services survive. The useful ones for an OS are:

- `GetTime()` / `SetTime()` for wall-clock time;
- `GetVariable()` / `SetVariable()` for UEFI variables (boot order, Secure Boot state);
- `ResetSystem()` for cold/warm reset;
- `QueryVariableInfo()`, capsule update services, and others.

The mapping problem:

- Before `SetVirtualAddressMap()`, runtime services execute with physical addresses. If the OS
  enables its own paging, it must keep the runtime-services code and data identity-mapped, or
  the firmware will fault when it dereferences its own physical pointers.
- `SetVirtualAddressMap()` (UEFI 2.11, Section 8.4) tells firmware the virtual addresses the OS
  assigned to runtime services. It may be called once. After that, runtime services use the
  virtual mapping and the OS must convert all runtime-services pointers.
- A kernel that maps runtime services at an arbitrary HHDM alias before `SetVirtualAddressMap`
  is asking for trouble: the firmware's internal absolute references still use physical
  addresses. Either identity-map them or call `SetVirtualAddressMap` properly.

Aster v1 policy: do not depend on runtime services. The first milestone can reset via the
keyboard controller or, once ACPI is parsed, the ACPI reset register; it does not need UEFI
variables or firmware time. Later, if variables or capsule updates are needed, add a dedicated
runtime-services window and call `SetVirtualAddressMap()` once, with a conversion table covered
by tests. This avoids carrying a fragile firmware mapping through the first boot.

## 7. Why a custom loader instead of GRUB

GRUB is a mature boot loader that understands file systems and kernel executable formats and can
load an arbitrary OS without recording the kernel's physical position [5]. It also provides a
menu, a command line, chainloading, and Multiboot/Multiboot2 support. Those are real advantages.

| Criterion | GRUB | Aster Boot (custom) |
| --- | --- | --- |
| Time to first boot | Fast; mature | Slower; new code and new bugs |
| Boot protocol | Multiboot2 or GRUB-specific; another ABI for the kernel | Aster boot-info ABI, designed for this kernel |
| Handoff mode on x64 UEFI | Typically a 32-bit protected-mode Multiboot handoff; a 64-bit kernel needs a trampoline | Native 64-bit UEFI handoff with the Microsoft x64 ABI |
| Control over memory map and framebuffer | Filtered through the boot protocol | Exact fields, exact flags, exact version |
| Pre-kernel TCB | GRUB plus its modules and config | Small loader, no shell, no menu, no FS stack beyond firmware |
| Secure Boot | GRUB shim/signature path exists but must be integrated | Loader must be signed or a shim used; not solved yet |
| Debuggability | Mature, widely tested | Entirely our code; easier to instrument, harder to blame on firmware |
| Ownership goal | External dependency | Part of the OS project |

The decision is custom for v1 because:

1. The kernel needs a boot-info ABI that carries exactly the Aster memory map, framebuffer,
   ACPI RSDP, kernel entry, and feature flags. Translating Multiboot2 into that structure is
   possible, but it adds a second protocol and a second set of assumptions.
2. UEFI x64 already hands off in 64-bit mode with a documented calling convention. A custom
   loader avoids the 32-bit trampoline that a Multiboot2 path typically needs.
3. The loader is small enough to audit and test. It does not need a shell, a menu, or a general
   file-system stack; it can use the firmware's Simple File System protocol.
4. The project goal includes owning the boot path. A custom loader is part of that goal, not
   just a means to an end.

The decision is not a claim that GRUB is bad. GRUB remains a useful debug fallback: if the
custom loader fails on a particular firmware, a GRUB/Multiboot2 image with the same kernel can
isolate whether the problem is in the loader or the kernel. The kernel's boot-info ABI should be
independent of the loader so that this fallback stays possible.

Other alternatives:

- **Limine**: a modern bootloader whose protocol is designed for higher-half kernels; OSDev notes
  that the Limine protocol only supports higher-half kernels by design [6]. It is a reasonable
  fallback, but it is still another protocol and another dependency.
- **UEFI stub in the kernel**: build the kernel as a UEFI application, Linux-style. This removes
  a stage, but the kernel then contains firmware-facing code, PE/COFF packaging, and UEFI
  protocol handling. It also makes non-UEFI boot paths harder. Aster keeps the loader separate.
- **BIOS/MBR**: legacy, no GOP, no native 64-bit handoff, more assembly. Rejected.

## 8. GPT, FAT32, and the EFI System Partition

UEFI 2.11, Chapter 5 defines the GUID Partition Table (GPT) [2]:

- LBA 0 contains a protective MBR.
- LBA 1 contains the GPT header with a CRC32, its own LBA, the backup LBA, the first usable LBA,
  the last usable LBA, the disk GUID, the partition-entry array LBA, the number of entries,
  their size, and the array CRC32.
- The backup header is at the last LBA.
- A GPT disk can have up to 128 partitions with the standard entry size.

The EFI System Partition (ESP) is a GPT partition with type GUID
`C12A7328-F81F-11D2-BA4B-00A0C93EC93B`. Microsoft's UEFI/GPT deployment guidance requires the
ESP to be formatted FAT32 [4]. UEFI firmware provides a FAT driver, which is why Aster Boot can
use `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` instead of implementing FAT32.

Proposed on-disk layout:

```
LBA 0                 protective MBR
LBA 1                 GPT header
LBA 2..33             partition entries (128 x 128 bytes)
ESP (FAT32)           \EFI\BOOT\BOOTX64.EFI      (Aster Boot)
                      \EFI\ASTER\ASTERKRN.ELF   (kernel)
                      \EFI\ASTER\BOOTINFO.TXT   (optional human-readable build info)
```

Notes:

- `\EFI\BOOT\BOOTX64.EFI` is the removable-media fallback path. For a fixed disk, firmware may
  instead use a `Boot####` variable. For v1, the fallback path is enough and avoids writing UEFI
  variables.
- The loader reads the kernel with the firmware's file protocol. It does not need to understand
  FAT directory entries, long file names, or cluster chains. This is a deliberate reduction in
  loader complexity.
- FAT32 has a maximum file size of 4 GiB minus 1 byte. The kernel image is far below that; if a
  future initrd or image outgrows it, the design must switch to a different container.
- The loader is read-only. It never writes to the ESP. Kernel logs go to serial, not to a file,
  until the kernel has its own file system.

## 9. OVMF and QEMU testing

OVMF is the Open Virtual Machine Firmware, an edk2-based UEFI firmware for virtual machines. Its
README lists x64 support, QEMU support, video, keyboard, IDE, CD-ROM, serial, the UEFI shell, and
optional network support [3]. It is the reference firmware for Aster's early boot tests.

QEMU exposes two relevant firmware options:

- `-bios file`: set the filename for the BIOS/firmware image [7].
- `-pflash file`: use the file as a parallel flash image [7].

OVMF's README recommends `-pflash` when UEFI variables must persist and states that a
pflash-backed variable store is required for Secure Boot builds; `-bios` only partially emulates
variables and non-volatile variables may be lost after reboot [3].

Proposed development command (not yet executed):

```
qemu-system-x86_64 \
  -machine q35 \
  -m 1024 \
  -smp 1 \
  -cpu max \
  -drive if=pflash,format=raw,unit=0,file=OVMF_CODE.fd,readonly=on \
  -drive if=pflash,format=raw,unit=1,file=OVMF_VARS.fd \
  -drive format=raw,file=aster.img \
  -vga std \
  -serial stdio \
  -display gtk
```

Firmware paths vary by distribution; `OVMF_CODE.fd` and `OVMF_VARS.fd` are placeholders. On
Windows, the QEMU install usually ships `edk2-x86_64-code.fd` and a variable template; use the
matching pair.

OVMF debug output:

- The default OVMF build writes debug messages to I/O port `0x402`.
- QEMU can capture them with
  `-debugcon file:ovmf.log -global isa-debugcon.iobase=0x402` [3].
- A RELEASE build disables debug messages; use a DEBUG build while bringing up the loader [3].

Proposed test matrix:

| Case | Firmware | RAM | Display | Purpose |
| --- | --- | --- | --- | --- |
| B1 | OVMF x64, DEBUG | 512 MiB | stdvga | Smallest practical memory map; catches assumptions about low memory |
| B2 | OVMF x64, DEBUG | 1024 MiB | stdvga | Default development case |
| B3 | OVMF x64, DEBUG | 4096 MiB | stdvga | Large memory map; tests 64-bit BARs and high memory |
| B4 | OVMF x64, RELEASE | 1024 MiB | stdvga | Debug-port absent; tests serial-only logging |
| B5 | OVMF x64, DEBUG | 1024 MiB | none / serial | Headless CI; serial log; GOP exposure depends on the QEMU display device |
| B6 | OVMF x64 + Secure Boot | 1024 MiB | stdvga | Unsigned loader rejection; future signing path |

`-cpu max` is convenient in TCG because it exposes NX, SMEP, SMAP, and other feature bits that
a minimal model may hide. Do not assume those features are present on real hardware; the kernel
must check CPUID.

Secure Boot (UEFI 2.11, Chapter 32, "Secure Boot and Driver Signing") [2]:

- OVMF is not shipped with Secure Boot keys installed by default; the user must enroll keys
  with the firmware UI or `OvmfPkg/EnrollDefaultKeys` [3].
- An unsigned Aster Boot will be rejected when Secure Boot is enabled. For bring-up, keep
  Secure Boot off. The production path is either signing the loader with a key enrolled in the
  firmware, or shipping a signed shim that verifies the loader. That is a follow-up, not a v1
  requirement.

## 10. Risks and open questions

1. **Map-key race**: the memory map can change between `GetMemoryMap` and `ExitBootServices`.
   Mitigation: no allocations in that window, bounded retry, log the failure.
2. **GOP mode mismatch**: firmware may expose only `PixelBltOnly` or a non-32-bit format.
   Mitigation: enumerate modes, reject unsupported ones with a clear diagnostic, and keep a
   serial fallback for CI.
3. **Framebuffer stride**: using width instead of `PixelsPerScanLine` produces skewed output.
   Mitigation: store the stride explicitly and test non-multiple-of-width resolutions.
4. **Runtime-services mapping**: calling runtime services after enabling paging without an
   identity map or `SetVirtualAddressMap` can fault inside firmware. Mitigation: do not use
   runtime services in v1.
5. **Firmware quirks**: OVMF is not a real machine. Real hardware may have different memory
   maps, GOP behavior, and `ExitBootServices` timing. Mitigation: test at least one real UEFI
   machine before declaring boot stable.
6. **Secure Boot**: unsigned loader is rejected. Mitigation: development uses Secure Boot off;
   production signing is a separate project.
7. **ACPI RSDP**: the configuration table pointer must be passed to the kernel; the kernel
   cannot find ACPI by scanning physical memory safely after it owns the map. Mitigation: record
   the RSDP physical address in the boot info.
8. **Loader integrity**: the loader reads the kernel from FAT32 without verifying a signature.
   Mitigation: add a hash/signature check before `ExitBootServices`; the boot info can carry the
   expected digest.

## 11. Sources and verification

Fetched successfully:

| Ref | Source | Notes |
| --- | --- | --- |
| [1] | UEFI Forum, "Specifications" (Wayback capture 2026-10-28): https://web.archive.org/web/20261028141608/https://uefi.org/specifications | Lists UEFI 2.11 (December 2024), 2.10 (August 2022), 2.10 Errata A (August 2024), PI 1.10 (July 2026) |
| [2] | UEFI Specification 2.11, official HTML (Wayback capture 2026-06-12): https://web.archive.org/web/20260612205110/https://uefi.org/specs/UEFI/2.11/ | Table of contents and chapter structure used for section references |
| [3] | Tianocore edk2, `OvmfPkg/README` (raw): https://raw.githubusercontent.com/tianocore/edk2/master/OvmfPkg/README | OVMF capabilities, QEMU options, debug port, Secure Boot notes |
| [4] | Microsoft Learn, "UEFI/GPT-based hard drive partitions": https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/configure-uefigpt-based-hard-drive-partitions | ESP must be FAT32; GPT partition guidance |
| [5] | GNU GRUB Manual, "Overview": https://www.gnu.org/software/grub/manual/grub/html_node/Overview.html | GRUB file-system and kernel-format flexibility |
| [6] | OSDev Wiki, "Higher Half Kernel" (Wayback capture 2026-09-11): https://web.archive.org/web/20260911100139/https://wiki.osdev.org/Higher_Half_Kernel | Notes Limine protocol is higher-half only |
| [7] | QEMU documentation, "Invocation": https://www.qemu.org/docs/master/system/invocation.html | `-bios` and `-pflash` descriptions |
| [8] | The rustc book, "Platform Support": https://doc.rust-lang.org/rustc/platform-support.html | `x86_64-unknown-uefi` is Tier 2 without host tools, std marked "?" |
| [9] | Intel SDM, Volume 3A: System Programming Guide, Part 1, order 253668-093US, September 2026: https://cdrdv2.intel.com/v1/dl/getContent/671190 | 4-level paging format and page-table entry layout |

Fetch failures and limitations:

- Direct `https://uefi.org/specs/UEFI/2.11/` and the official 2.11 PDF returned HTTP 403 to the
  fetch tool. The HTML table of contents was read through the Wayback capture above. The exact
  wording of individual spec paragraphs was not machine-quoted; section references are to the
  chapter structure shown in that table of contents.
- The UEFI 2.10 PDF also returned HTTP 403. No 2.10-specific wording is quoted.
- OVMF README is fetched from the moving `master` branch. Pin a specific edk2 tag for
  reproducible tests.

Cross-links: [ADR 0002](../decisions/0002-boot-and-firmware.md),
[Kernel architecture](kernel-architecture.md),
[x86_64 memory and interrupts](x86_64-memory-and-interrupts.md),
[ADR 0004 address space](../decisions/0004-address-space-layout.md).
