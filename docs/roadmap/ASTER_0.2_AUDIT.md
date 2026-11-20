# Aster OS 0.2 - Repository Audit

Date: 2026-11-20
Branch: `milestone/aster-0.2-core`
Baseline tag: `v0.1.0-baseline` (commit `fc8caf3`)
Scope: the complete tree at the start of 0.2 work, inspected directly rather
than inferred from documentation.

This audit records what is real, what is partial, what is missing, and which
defects constrain the 0.2 design. Every claim below was checked against the
code or a live QEMU run. Line numbers refer to the baseline tree.

## 1. Baseline measurements

| Check | Result |
|---|---|
| `python tools/build.py` | PASS, 512 MiB GPT/FAT32 image verified |
| `python tools/test.py` | PASS, all host tests and the boot smoke test |
| `tools/ui_test.py` | PASS, desktop and Terminal screenshots |
| `tools/pointer_test.py` | PASS, dock, notification centre, Control Centre |
| `tools/app_probe.py` | PASS, nine applications captured |
| Benchmark (one run, QEMU TCG) | Boot handoff 1.76 s, kernel ready 1.98 s, first frame 5.30 s |
| In-guest desktop init | 2251 ms, first frame 58.0 ms |
| Idle frame time | 52-60 ms (about 17-19 fps at 1280x800) |
| Frame time with several windows | about 80 ms |
| Free RAM after desktop init | 404 MiB of 510 MiB |
| Kernel heap | 1024 KiB mapped, 72 KiB used at boot |

The desktop is a single ring-3 process (PID 1) with a 48 MiB user heap and a
64 KiB user stack. The kernel has no second user process, no IPC and no
persistent storage.

## 2. Verified functionality

These subsystems exist and are exercised by the build, the boot smoke test, or
a UI probe:

- **Boot**: custom UEFI loader, GPT/FAT32 ESP, ELF validation, page-table
  construction, `ExitBootServices`, higher-half entry, versioned `BootInfo`.
- **CPU tables**: GDT/TSS, 256-vector IDT, IST double-fault stack, DPL3 `int
  0x80` gate, PIC remap, 1 kHz PIT, FXSAVE/FXRSTOR context switching.
- **Memory**: bitmap PMM over usable RAM, 4-level paging with per-process PML4,
  NX and W^X segment flags, kernel heap with self-test, user-pointer copy
  helpers, user address-space self-test.
- **Scheduling**: preemptive round-robin over user threads, cooperative kernel
  threads, idle thread, `sleep_ms`, `yield`, per-thread 32 KiB kernel stacks.
- **Ring 3**: ELF loading from boot modules, private page tables, NX stack/heap,
  framebuffer mapping, 24 implemented syscalls, user page-fault containment.
- **Storage (volatile)**: in-memory ramfs with directories, 4 MiB file cap,
  per-process fd table, path-based open/stat/mkdir/unlink.
- **Input/display**: PS/2 keyboard and mouse, GOP framebuffer, software
  compositor, window manager, dock, menu bar, Spotlight, Control Centre,
  notifications, three virtual desktops.
- **Applications**: Terminal, Files, Settings, Calculator, Monitor, Text, Notes,
  Calendar, Wallpapers. All run inside the desktop address space.
- **Tests**: host image verifier, boot smoke test, UI/app/pointer/window probes,
  benchmark harness.

## 3. Partial implementations and stubs

| Area | State | Evidence |
|---|---|---|
| Process creation | Missing | `SPAWN` (21) falls through to `ENOSYS` (`kernel/src/syscall.rs:67`); the only loader path is `user::spawn_module` for the boot module |
| Process wait/exit status | Missing | `Process.exit_code` is written at `kernel/src/sched.rs:617` and never read; processes are destroyed in `reap` (`sched.rs:457-470`) |
| IPC | Missing | `CHANNEL_CREATE/SEND/RECV` (24-26) are `ENOSYS`; no channel objects, handles or wait queues |
| Persistence | Missing | ramfs only; settings and files vanish on shutdown |
| Block devices | Missing | no PCI enumeration, no ATA/AHCI, no VirtIO block |
| VFS mounts | Missing | one global ramfs; no mount table, no second filesystem |
| User exception containment | Partial | only `#PF` from ring 3 is contained (`kernel/src/exceptions.rs:40-44`); every other user exception halts the machine (`exceptions.rs:94-97`) |
| Per-process limits | Missing | no page quota, process quota, or handle quota |
| User thread cleanup | Partial | reaping only runs after a context switch (`sched.rs:550-554`) |
| `PROC_LIST` | Partial | reports `threads.max(1)` and a hardcoded state (`sched.rs:410-413`) |
| Page-table accounting | Stub | `vmm::total_mapped_pages` always returns 0 (`kernel/src/mm/vmm.rs:347-349`) |
| SMEP/SMAP/UMIP | Missing | `cpu.rs` enables only OSFXSR/OSXMMEXCPT |
| PCI/ACPI/APIC/SMP | Missing | no enumeration; single CPU; 8259 only |
| Networking/audio/USB | Missing | no drivers; the UI states this |
| Users/permissions | Missing | single implicit user; mode bits are informational |
| Damage tracking | Missing | full-frame composition every frame |

## 4. Defects found during the audit

Severity: **S1** blocks a milestone, **S2** correctness/security gap, **S3**
quality or diagnosability.

### 4.1 Kernel and memory

1. **S1 - Only page faults are contained.** A `ud2`, `int3`, divide error, or
   general-protection fault in ring 3 reaches the fatal path and halts the
   whole OS (`kernel/src/exceptions.rs:46-97`). One buggy application can stop
   the machine.
2. **S2 - Framebuffer MMIO is freed as if it were RAM.** The desktop maps the
   GOP framebuffer `PTE_USER` into its PML4; `destroy_user_pml4` frees every
   present PTE (`kernel/src/mm/vmm.rs:226-230`), including the framebuffer
   frames. `pmm::free_frame` does not check that the frame is below the managed
   limit (`kernel/src/mm/pmm.rs:149-162`), so the free counter is corrupted.
3. **S2 - Partial mappings leak.** `map_segment` and `map_stack` free only the
   frame that failed, not the pages already mapped (`kernel/src/user.rs:98-143`);
   `spawn_module` leaks the PML4 and all mapped frames on any later failure
   (`user.rs:152-167`).
4. **S2 - `VM_ALLOC` is unbounded and overflow-prone.** `length + PAGE_SIZE - 1`
   and `pages * PAGE_SIZE` can overflow; a failed allocation leaves earlier
   pages mapped and does not advance the cursor (`kernel/src/mm/vmm.rs:260-289`,
   `kernel/src/syscall.rs:139-150`). The heap limit is `USER_TOP`, so a large
   heap can collide with the user stack (`vmm.rs:272`).
5. **S2 - User copy helpers do not check permissions.** `copy_from_user` and
   `copy_to_user` translate pages but do not require `PTE_USER` or `PTE_WRITE`
   (`kernel/src/mm/vmm.rs:291-333`). Today this is mostly self-inflicted, but it
   is a defense-in-depth gap and blocks safe IPC.
6. **S2 - Path copies require 256 readable bytes.** `read_path` copies a fixed
   256-byte buffer before looking for NUL (`kernel/src/syscall.rs:182-195`);
   a valid path at the end of a mapped page is rejected.
7. **S2 - Kernel heap is 1 MiB while a single read may allocate 1 MiB.**
   `READ` accepts up to 1 MiB (`syscall.rs:238`) and `MAX_FILE_SIZE` is 4 MiB
   (`kernel/src/fs.rs:44`); a large file can exhaust the kernel heap.
8. **S3 - `total_mapped_pages` is unimplemented** (`vmm.rs:347-349`).
9. **S3 - No guard pages on 32 KiB kernel stacks** (`sched.rs:19`).

### 4.2 Scheduler and processes

10. **S1 - Reaping is skipped when no context switch occurs.** `schedule`
    returns early when `next == current` (`sched.rs:516-520`); reaping only runs
    after a real switch (`sched.rs:550-554`). The last exiting process can leak
    its thread, stack, process, page tables and frames permanently.
11. **S1 - No zombies, no parent links, no wait.** A process is destroyed as
    soon as its last thread exits, so `WAIT` has nothing to observe and the exit
    status cannot be delivered (`sched.rs:445-472`).
12. **S2 - Threads are published before their process is registered.**
    `create_thread_raw` pushes the thread into the run list
    (`sched.rs:236`) before `register_process` assigns the pid and pushes the
    process (`sched.rs:292-296`). Latent race once spawn is callable at runtime.
13. **S2 - Blocking is timestamp-only.** `Blocked` with `wake_at == 0` is
    immediately marked Ready (`sched.rs:495`, `sched.rs:641`); there is no
    `wake(tid)` and no wait queue. IPC cannot block correctly without this.
14. **S2 - Kernel-thread yield leaves interrupts disabled.**
    `yield_now`/`sleep_ms` called from a kernel thread return with IF=0
    (`sched.rs:571-605`).
15. **S3 - Round-robin always skips the current thread** (`sched.rs:421-443`),
    so one runnable user thread drops to idle every quantum.
16. **S3 - New threads enter with the wrong SysV stack alignment**
    (`sched.rs:149-164`).
17. **S3 - Wake latency up to one quantum** because sleepers are only made Ready
    and `schedule` still waits for `NEED_RESCHED` (`sched.rs:637-665`).

### 4.3 Loader and syscall surface

18. **S1 - ELF bounds arithmetic can overflow.** `offset + filesz`,
    `vaddr + memsz` and `phoff + phentsize * phnum` are unchecked
    (`kernel/src/user.rs:57,71,77`). Once arbitrary files can be spawned, a
    crafted ELF becomes a kernel out-of-bounds read or a long denial-of-service
    loop.
19. **S2 - RWX segments are accepted.** `PF_W|PF_X` maps a writable executable
    page (`user.rs:103-109`), contradicting the documented W^X guarantee.
20. **S2 - No segment overlap or stack-collision checks.** A segment may claim
    the stack window or the heap cursor (`user.rs:77-87`, `vmm.rs:16-18`).
21. **S2 - Entry is not required to be executable.** The loader only checks the
    address range (`user.rs:92-94`).
22. **S2 - Any process can power off the machine.** `SHUTDOWN` has no capability
    check (`syscall.rs:448-457`).
23. **S3 - `OPEN` does not validate flag combinations** and `SEEK` can overflow
    `base + offset` (`syscall.rs:205-229`, `syscall.rs:307`).
24. **S3 - `UNLINK` is recursive with no permissions or inode refcounts**
    (`kernel/src/fs.rs:387-395`).

### 4.4 Storage, drivers, documentation

25. **S1 - No block device and no mount layer.** The FAT32 builder and verifier
    exist only on the host (`tools/image.py`); the kernel cannot read a sector
    other than through the UEFI loader.
26. **S2 - MMIO BARs are not mapped.** The direct map covers RAM plus the GOP
    framebuffer only (`boot/src/main.rs:504-523`); a device BAR such as the
    q35 AHCI controller at `0x81084000` must be mapped explicitly.
27. **S3 - Stale seeded settings.** `/home/user/settings.conf` still says
    `theme=dark` and `wallpaper=aurora` (`kernel/src/fs.rs:224-226`), although
    the shipped desktop is light-only.
28. **S3 - Documentation overstates two guarantees.** The threat model says a
    ring-3 fault kills only the faulting thread and that W^X is enforced; only
    page faults are contained and RWX segments are accepted
    (`docs/security/THREAT_MODEL.md:129-135`).
29. **S3 - Mojibake in `docs/architecture/IPC.md`** ("16 channels ... 64 KiB/s").

## 5. Dependencies and sequencing

The three candidate first milestones have different dependency graphs:

```
Persistence:  PCI/AHCI -> block layer -> partition/FAT -> VFS mount -> apps
Process isolation: loader hardening -> process table -> spawn/wait/zombies
IPC:          process isolation -> wait queues/handles -> channels -> services
Services:     IPC + persistence
```

Conclusions:

- **Persistence is independent of the process model.** It can be built and
  tested without touching the scheduler or the desktop's app model. It directly
  fixes the most visible limitation (data loss) and creates the on-disk home
  that a future storage service will own.
- **Process isolation and IPC are tightly coupled.** `WAIT` needs zombie
  processes and a wake primitive; IPC needs the same wait primitive and
  per-process handles. They should be one milestone, after persistence.
- **Service extraction must wait for both.** A storage service needs IPC and a
  working block/FAT stack; a display service needs shared memory and a window
  protocol.

Chosen order for 0.2:

1. **M1 - Persistent home volume** (this milestone).
2. **M2 - Process isolation and lifecycle** (`SPAWN`, `WAIT`, zombies, quotas,
   guard pages, full exception containment).
3. **M3 - IPC channels** (blocking send/recv, handles, close/hangup, echo
   tests) and a bootstrap channel in `SPAWN`.
4. **M4 - First system service** (storage service over IPC, with the kernel
   filesystem retained as a fallback).
5. **M5 - Security and robustness hardening** (SMEP/SMAP/UMIP, capabilities,
   audit trail, stack guards).
6. **M6 - Performance** (damage tracking, lazy heap growth, frame cache).

## 6. Risks for M1

| Risk | Mitigation |
|---|---|
| AHCI DMA structure bugs corrupt memory | Dedicated frames, 1 KiB alignment, IDENTIFY before use, timeouts, no interrupts (polling) |
| FAT32 writes corrupt the volume | Write a fresh data image for each test run; host verifier reads it back; mount read-only if the BPB fails validation |
| LFN complexity delays the milestone | Ship 8.3 names first, document the limit, add LFN as a follow-up within the milestone only if the core passes |
| Boot disk is damaged by a stray write | Data lives on a second disk (`if=ide,index=1`, 512 MiB); the boot disk is only ever read |
| QEMU probe scripts diverge | Centralize QEMU arguments in `tools/run.py` and have probes import them |
| Kernel heap exhaustion | No whole-FAT cache; sector-sized buffers; cap file reads; document limits |
| Persistence test depends on a dirty image | Tests create a fresh data image; the persistence test deliberately reuses one across two boots |

## 7. What this audit does not claim

- It does not claim the filesystem is reliable under power loss; M1 has no
  journal.
- It does not claim FAT32 long-name support until it is implemented and tested.
- It does not claim process isolation or IPC; both remain explicitly absent
  from the UI and documentation until M2/M3 land.
