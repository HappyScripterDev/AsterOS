# Security Threat Model

Date: 2026-11-20. Scope: the developer-preview system in this repository. This
document states what is protected, what is not, and how each claim is verified.
It is deliberately conservative: an unverified mitigation is not a feature.

## 1. Assets

1. Kernel integrity and control flow.
2. Process memory confidentiality and integrity.
3. Files in the ramfs (currently all-or-nothing; no per-user data).
4. The desktop session (input, screen content, notifications).
5. Boot integrity (loader, kernel image, modules).
6. Future: credentials, packages, updates, user documents.

## 2. Adversary model

In scope:

- A malicious or buggy ring-3 application (including a built-in app with a
  rendering or parsing bug).
- A user at the keyboard attempting to access another process's memory or
  escalate privileges through syscalls.
- A corrupted or malicious file supplied to an application (e.g. a text file
  opened by the editor).
- A malicious disk image or boot module if the firmware boots it (physical
  access / supply chain).

Out of scope for this milestone:

- Remote network attackers (no network stack exists).
- DMA attacks (no IOMMU, no external device drivers).
- Microarchitectural side channels (Spectre/Meltdown class).
- Firmware/CPU compromise below UEFI.
- Multi-user isolation (single implicit user).
- Denial of service by resource exhaustion is acknowledged, not prevented.

## 3. Trust boundaries

```
+---------------------------- ring 3 ----------------------------+
|  Desktop (PID 1) + apps in the same address space              |
|   - owns framebuffer mapping, reads input, calls syscalls      |
+------------------------------|---------------------------------+
                               | int 0x80 (validated pointers)
+------------------------------v---------------------------------+
|  Kernel: PMM/VMM/heap, scheduler, ramfs, PS/2, time            |
|   - ring 0, full physical memory via direct map                |
+------------------------------|---------------------------------+
                               | BootInfo (validated magic/version)
+------------------------------v---------------------------------+
|  Aster Boot (UEFI application)                                 |
|   - runs with firmware services, then ExitBootServices         |
+------------------------------|---------------------------------+
                               | UEFI firmware / ESP files
+------------------------------v---------------------------------+
|  OVMF / platform firmware, disk image                          |
+----------------------------------------------------------------+
```

Spawned processes have private address spaces and can exchange messages over
kernel channels with per-process handles. Built-in apps still share the
desktop address space; a bug in one can read another's memory. This is an
explicit limitation, not a hidden one.

## 4. Threats and mitigations

### 4.1 Boot chain

| Threat | Mitigation today | Gap |
|---|---|---|
| Modified kernel/module on disk | None beyond firmware boot policy | No signatures, no measured boot, no rollback protection |
| Malicious ELF in the ESP | Loader validates ELF64/x86_64, bounds, page alignment, segment sizes | No signature or hash pinning |
| Invalid BootInfo tricking the kernel | Magic/version/direct-map/descriptor-size checks; halt on mismatch | BootInfo is trusted after validation; no authentication |
| Firmware runtime abuse | `ExitBootServices` is called; runtime services are not used | UEFI runtime services are not mapped after exit |
| Loader writing outside allocated pages | All segments use UEFI-allocated pages; sizes checked | A loader bug could still corrupt firmware memory before exit |

### 4.2 Kernel

| Threat | Mitigation today | Gap |
|---|---|---|
| User pointer passed to a syscall | Every pointer is translated per page and range-checked; copies go through the direct map | No SMAP/SMEP; a kernel bug could still dereference a user pointer |
| Executable user data | NX on stacks/heaps; W^X for user segments | Kernel direct map is NX, but kernel text/data permissions rely on loader segment flags |
| Privileged instruction in ring 3 | Ring 3 CS/SS, no IOPL, TSS without I/O bitmap | No UMIP, no CR4.SMEP/SMAP |
| Exception causing kernel crash | User page faults terminate the process; fatal kernel faults print and halt | No recovery/restart of the kernel |
| Resource exhaustion | Bounded queues, 4 MiB file cap, 32 GiB frame bitmap | No per-process quotas, no OOM killer, no memory cgroups |
| Kernel stack overflow | 32 KiB per thread, TSS RSP0 | No guard pages; stack overflow corrupts adjacent memory |
| Race conditions | Single CPU; interrupt-safe locks; preemption only for user threads | SMP is not supported; locks are not designed for it yet |

### 4.3 Drivers

| Threat | Mitigation today | Gap |
|---|---|---|
| PS/2 event flood | 256-event ring drops oldest instead of growing | No rate limiting or per-device quotas |
| Mouse coordinates out of range | Clamped to framebuffer dimensions | Firmware-provided stride/resolution is trusted after validation |
| Framebuffer writes | Only the desktop maps it; user VA, NX, write-only | No IOMMU; a kernel bug could map arbitrary physical memory |
| DMA from a disk controller | AHCI PRDTs point only at dedicated PMM frames; no user pointer is programmed | No IOMMU; a driver bug could DMA elsewhere |
| Serial input | Not used as a command channel | No authentication if it becomes one |

### 4.4 Filesystem

| Threat | Mitigation today | Gap |
|---|---|---|
| Path traversal (`..`) | Resolver clamps at root | No per-process root/jail |
| Access control | None: the ramfs and the FAT32 home are global; 0644/0755 are informational | No uid/gid enforcement, ACLs, or capabilities |
| Symlink attacks | Symlinks are not implemented | Future AFS must handle them carefully |
| Disk corruption | GPT header CRC, FAT geometry checks, bounded chain walks, `EIO` on malformed metadata | No journal/checksums; a torn multi-sector write can truncate a file |
| Settings corruption | v1 validation, per-key defaults, `SETTINGS.BAD`, `SETTINGS.BAK`, temp+rename+flush | No journal; power loss between renames can fall back to the backup |
| Service impersonation | configd only accepts messages on its inherited bootstrap channel; handles are per-process | No capability model or per-service identity |
| Hung service | IPC receive timeout; desktop falls back to direct settings I/O | No `KILL`; a hung service cannot be forcibly removed |
| Data loss on power-off | `/home/user` is persistent when the data disk is present | System files remain volatile; writes are synchronous but not atomic |
| Boot image corruption | The kernel only reads the ESP; user data is on a separate disk | Physical access can still replace the boot image |

### 4.5 Desktop and applications

| Threat | Mitigation today | Gap |
|---|---|---|
| App reads another app's data | Spawned processes are isolated; IPC handles are per-process | Built-in apps still share the desktop address space |
| Handle forgery / message flood | Handles validated against the caller's table; 8 x 256 B queues, 32 channels; receive timeout | No capability layer; send has no timeout |
| Malicious file crashes the editor | Strings are UTF-8 lossy; sizes capped | No fuzzing; no parser sandboxing |
| Fake UI actions | Controls call real syscalls or show explicit "not implemented" text | Store/Installer/Update/Browser are absent by design |
| Screen capture by an app | Framebuffer is mapped only into the desktop | Any in-process app can read the desktop's buffer |
| Notification spoofing | Apps can request notifications through the shell | No per-app identity or signature |

### 4.6 Users, packages, updates

Not implemented. There are no passwords, login sessions, package signatures,
update metadata, or rollback. The UI does not pretend otherwise: the Settings
sections state "Not implemented" and no Store/Updater app exists.

## 5. Verified security properties

These are properties the current tests actually exercise:

1. Any exception raised by ring 3 is contained: the kernel logs pid/tid/vector
   and terminates that process; the desktop continues. Page faults, `ud2`,
   divide errors and general-protection faults are covered by the same path.
2. User pages require U/S at every paging level; the initial bug produced a
   protection fault and was fixed in `vmm::map_page`.
3. W^X for user segments is enforced by the loader: writable-and-executable
   segments are rejected, non-executable segments get NX, and CR0.WP is set.
4. Syscall buffers are validated page by page; the leaf must be user-accessible
   (and writable for output), otherwise `EINVAL` is returned without a fault.
5. The desktop cannot execute privileged instructions; all hardware access is
   through syscalls and the framebuffer mapping.

These properties are not a formal proof. They are checked by code review,
targeted tests, and deliberate fault injection during development. No
third-party security audit has been performed.

## 6. Accepted risks for the developer preview

- Single user, no authentication; anyone at the keyboard controls the machine.
- No sandboxing; a desktop app bug compromises the session.
- No signed boot or updates; physical access is game over.
- No ASLR/KASLR; addresses are predictable.
- No SMEP/SMAP/UMIP; kernel pointer bugs are more dangerous.
- No resource quotas; a malicious app can exhaust memory or the event queue.
- The persistent home is world-readable and world-writable, uses 8.3 names, and
  has no journal; a power loss can truncate a file.
- No audit log exposed to user space.

Each risk has an owner and a milestone in `docs/ROADMAP.md`. Security claims in
the UI are limited to what is implemented (ring 3, NX, pointer validation) and
the Settings app explicitly lists missing protections.

## 7. Verification plan

Before calling the system "secure" beyond a preview:

1. Enable SMEP/SMAP/UMIP and add tests that attempt user-page execution and
   user-pointer dereference from the kernel.
2. Add KASLR/ASLR and verify entropy at boot.
3. Implement per-process capabilities and enforce filesystem permissions.
4. Move apps to separate processes; add IPC fuzzing and handle-forgery tests.
5. Add signed boot/update metadata with a documented key ceremony.
6. Run a third-party review of the syscall ABI, paging, and ELF loader.
7. Add fault injection for OOM and corrupted filesystem images.
