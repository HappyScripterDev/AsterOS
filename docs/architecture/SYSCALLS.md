# System Calls

ABI: `aster_abi::sys`, version 1. Numbers are never reused. Arguments follow the
platform ABI: `rax` = number, `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`; the result
is in `rax`, with negative values as errors. The `int 0x80` gate is DPL 3 and
runs with interrupts disabled.

User pointers are validated page by page with `vmm::copy_from_user` /
`copy_to_user`; the leaf must be present, user-accessible and writable when the
kernel writes. Missing mappings return `EINVAL`, never fault the kernel.

| # | Name | Arguments | Result | Status |
|---:|---|---|---|---|
| 1 | EXIT | status | never returns | Implemented |
| 2 | WRITE | fd (1/2), ptr, len | bytes written | Implemented, serial only |
| 3 | GETPID | - | pid | Implemented |
| 4 | YIELD | - | 0 | Implemented |
| 5 | SLEEP_MS | ms | 0 | Implemented, blocks |
| 6 | TIME_MS | - | uptime ms | Implemented |
| 7 | FB_INFO | out ptr | bytes | Implemented |
| 8 | FB_MAP | - | user VA | Implemented |
| 9 | INPUT_POLL | out ptr, max | events | Implemented |
| 10 | VM_ALLOC | length | user VA | Implemented, 64 MiB per call |
| 11 | LOG | ptr, len | bytes | Implemented |
| 12 | OPEN | path, flags | fd | Implemented |
| 13 | READ | fd, ptr, len | bytes | Implemented, 1 MiB cap |
| 14 | WRITE_FD | fd, ptr, len | bytes | Implemented |
| 15 | CLOSE | fd | 0 | Implemented |
| 16 | SEEK | fd, offset, whence | offset | Implemented |
| 17 | READDIR | fd, ptr, max | entries | Implemented |
| 18 | STAT | path, out | 0 | Implemented |
| 19 | UNLINK | path | 0 | Implemented |
| 20 | MKDIR | path | 0 | Implemented |
| 21 | SPAWN | path | child pid | Implemented (`ECHILD`/`EPERM`/`ENOENT`) |
| 22 | WAIT | pid, status ptr | child pid, status written | Implemented; `pid 0` waits for any child |
| 23 | SYSTEM_INFO | out ptr | bytes | Implemented |
| 24 | CHANNEL_CREATE | ptr to `u32[2]` | handles written | Implemented |
| 25 | CHANNEL_SEND | handle, ptr, len | bytes | Implemented, blocks when full |
| 26 | CHANNEL_RECV | handle, ptr, cap, timeout ms | bytes | Implemented, blocks when empty; ETIMEDOUT |
| 27 | SET_THEME | - | 0 | **ENOSYS** (appearance is fixed to light) |
| 28 | SHUTDOWN | - | halts | Implemented, no capability check |
| 29 | RTC | out ptr | 0 | Implemented |
| 30 | PROC_LIST | out ptr, max | count | Implemented |
| 31 | STORAGE_INFO | out ptr | 0 | Implemented (M1) |
| 32 | CHANNEL_CLOSE | handle | 0 | Implemented |
| 33 | STORAGE_DETAILS | out ptr | 0 | Implemented (0.3.0) |
| 34 | RENAME | old path, new path | 0 | Implemented, same directory |
| 35 | LOG_READ | ptr, cap | bytes | Implemented |
| 36 | SYNC | - | 0 | Implemented (AHCI flush) |

## Error codes

`EPERM`, `ENOENT`, `EBADF`, `ENOMEM`, `EINVAL`, `EIO`, `EEXIST`, `ENOTDIR`,
`EISDIR`, `ENOSPC`, `ENAMETOOLONG`, `ECHILD`, `ESRCH`, `EPIPE`, `ETIMEDOUT`,
`ENOSYS`. `ETIMEDOUT` is reserved for future timers.

## Validation rules

- Path copies read one byte at a time until NUL or 256 bytes, so a valid short
  path at the end of a mapped page is accepted.
- `READ`/`WRITE_FD` cap the kernel-side buffer. The FAT driver returns short
  reads of at most 64 KiB; user space loops.
- `VM_ALLOC` rejects zero, more than 64 MiB in one call, overflow, and addresses
  that would cross the guard below the user stack.
- File fds are per process; slots 0-2 are reserved, 3-23 are usable.
- `SHUTDOWN` is callable by any process; a capability check is planned for M5.

## Blocking

`SLEEP_MS`, `WAIT` (until a child becomes a zombie), `CHANNEL_SEND` (queue
full), `CHANNEL_RECV` (queue empty, optionally until a timeout) and `EXIT`
(never returns) block today. A blocking
syscall must set the thread state and call the scheduler itself; the gate
already runs with interrupts disabled. IPC will add `block_on`/`wake`.
