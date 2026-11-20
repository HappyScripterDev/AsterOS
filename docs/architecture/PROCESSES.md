# Processes and Isolation

Status: **0.3.0** (2026-11-20). Independent ring-3 processes can be spawned,
waited for, and killed by faults without taking down the desktop. `configd` is
a long-lived child supervised by the desktop. The built-in apps still share the
desktop process; extracting them is a later milestone.

## 1. Address spaces

Every process receives a fresh PML4 from `vmm::create_user_pml4()`:

- entries 256..512 (kernel image, direct map, heap, MMIO) are shared by value;
- entries 0..255 (user space) start empty;
- ELF segments are mapped user/W^X at their link addresses;
- a 64 KiB NX stack is mapped below `USER_STACK_TOP`;
- the bump heap starts at `USER_BASE + 0x1000_0000`.

Anonymous frames carry the software bit `PTE_OWNED`; address-space destruction
frees only owned frames. Device MMIO and future shared mappings are not freed.

## 2. Lifecycle

```
SPAWN(path)
  -> read ELF from the VFS, or a boot module with the same stem
  -> validate ELF (64-bit, bounds, page alignment, W^X, entry in RX, no overlap)
  -> create PML4, map segments and stack
  -> register_process(parent = current pid)
  -> child becomes Ready

child EXIT(status) / fault
  -> thread Dead, status stored, threads_alive -= 1
  -> last thread: process Zombie, wake parent blocked in WAIT
  -> scheduler frees the dead kernel stack

parent WAIT(pid, &status)
  -> reap the zombie, destroy its PML4, copy status, return child pid
  -> no zombie yet: block on the wait sentinel and yield

parent exits
  -> children become orphans; the next reap destroys their zombies
```

`SPAWN` and `WAIT` are syscalls 21 and 22. `WAIT(0, &status)` waits for any
child. The status is written through a validated user pointer, so negative exit
statuses (for example `-11` for a fault) cannot be confused with errno values.

## 3. Fault containment

Every ring-3 exception is contained, not only page faults: page fault, invalid
opcode, breakpoint, divide error, general protection, alignment check, stack
fault and SIMD faults all terminate the offending process with status `-11`.
The kernel logs pid/tid/vector/rip and continues. `faultprobe` deliberately
writes to a kernel address to exercise this path.

## 4. Limits and policy

- Maximum 24 live processes (`EPERM` beyond that).
- No per-process memory quota; a child can allocate until physical memory is
  exhausted. A quota is planned for M5.
- No `KILL`; processes exit themselves or fault.
- No `argv`/environment; children start with an empty stack.
- No inheritance of file descriptors or handles; `SPAWN` passes exactly one
  bootstrap channel handle to the child.
- `configd` is restarted by the desktop supervisor; there is no `KILL` for a
  hung child.
- `WAIT` blocks the calling thread. The desktop has one UI thread, so a long
  child freezes the shell until it exits; the probe commands are short. An
  asynchronous event/service API is required before apps use blocking waits.
- No ASLR; user images link at 4 MiB and the stack/heap addresses are fixed.
- The framebuffer mapping is still available to any process that calls
  `FB_MAP`; a display capability is planned with the display service.

## 5. Evidence

`tools/isolation_test.py` boots the system, opens the Terminal, runs the
`isolation` command, and asserts the serial log contains:

- `spawn: pid 2 'isoprobe' ... parent 1`
- `isoprobe: private page verified, exiting with 42`
- `isoprobe: ...` plus `exit: pid 2`
- `spawn: pid 3 'faultprobe'`
- `user exception: pid 3` for the kernel write
- `isolation: pid 2 status 42`
- `isolation: pid 3 status -11`
- `DESKTOP-READY` and no kernel panic

The boot memory self-test also creates two address spaces, maps the same virtual
address in both to different frames, verifies that writes stay private, checks
that kernel addresses are rejected as user buffers, and verifies that teardown
returns the frames.

## 6. Follow-ups

1. Per-process page quotas and a global reserve.
2. `KILL`/signal delivery and a multi-threaded process model.
3. File-descriptor and channel inheritance, with an explicit bootstrap handle.
4. Guard pages between the stack, heap and image.
5. ASLR for the image, stack and heap.
6. Moving the built-in apps into separate processes (requires IPC and surfaces).
