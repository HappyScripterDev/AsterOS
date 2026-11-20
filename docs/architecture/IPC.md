# Inter-Process Communication

Status: **M3 core shipped** (2026-11-20). Two isolated processes exchange
messages through kernel channels with bounded queues and blocking receive. The
Terminal `ipc` and `ipchup` commands exercise the happy path, a forged handle
and a hangup.

## 1. Model

A **channel** has two endpoints. Each endpoint is a handle in a process's table
and owns a bounded FIFO of inline messages. Sending on one endpoint enqueues for
the other endpoint; the channel is bidirectional because each endpoint can both
send and receive.

```
process A                          process B
 handle 3 -> endpoint 0  <---->  endpoint 1 <- handle 3
                queue for B        queue for A
```

Handles are per-process indices (3..15). They are never global pointers, pids or
kernel addresses. A message carries only bytes; the kernel records the sender
pid and a per-channel sequence number internally for diagnostics.

## 2. API

| Call | Arguments | Result |
|---|---|---|
| `CHANNEL_CREATE` (24) | pointer to `u32[2]` | writes the two handle indices, returns 0 |
| `CHANNEL_SEND` (25) | handle, pointer, length | bytes queued; blocks if the peer queue is full |
| `CHANNEL_RECV` (26) | handle, pointer, capacity | bytes copied; blocks if empty |
| `CHANNEL_CLOSE` (32) | handle | 0, or `EBADF` |

Limits:

- 256-byte inline payload, 8 messages per queue, 32 live channels.
- 16 handle slots per process, indices 3..15 (0..2 are reserved).
- A send larger than 256 bytes returns `EINVAL`; a receive buffer smaller than
  the next message returns `EINVAL` without consuming it.

## 3. Bootstrap

`SPAWN(path, bootstrap_handle)` duplicates one endpoint into the child's first
free handle slot, which is 3 for a fresh process. The parent keeps its own
handle to the other endpoint. The convention is documented so a child can use
handle 3 without an argv protocol:

```
parent: create -> handles 3 (A), 4 (B)
parent: spawn("/home/user/ipcprobe.elf", 4)
child:  receives B at handle 3
```

The duplicate increments the endpoint's reference count. Closing the parent's
copy does not hang up while the child still holds its own reference.

## 4. Blocking and wake-up

`CHANNEL_RECV` on an empty queue with the peer still open:

1. records the current tid in the endpoint's receive-waiter list,
2. marks the thread `Blocked` with `wake_at = u64::MAX` (the event sentinel),
3. calls the scheduler.

A successful send pops one receive waiter and marks it `Ready`; a successful
receive pops one send waiter. `schedule` then switches to the woken thread.

`CHANNEL_SEND` blocks while the peer queue is full. The receiver wakes one
blocked sender after draining a message.

The syscall gate runs with interrupts disabled. The channel lock is dropped
before `schedule()` and no kernel lock is held across a user copy. The single
CPU means no other process can run between the state check and the block, so a
wakeup cannot be lost.

## 5. Close, exit and errors

- Closing a handle drops its endpoint reference.
- The last reference closes that endpoint and wakes every waiter on both
  endpoints. A blocked receiver then returns `EPIPE` once the queue is empty.
- `exit_current` closes all of the process's handles before the process becomes
  a zombie. A peer blocked on a dead process therefore wakes with `EPIPE`
  instead of waiting forever.
- `EBADF` for an unknown or closed handle; `EPIPE` for a closed peer;
  `EINVAL` for bad sizes or user pointers; `ENOSPC` when the channel table is
  full.

There is no timeout yet: `ETIMEDOUT` is reserved. A malicious process cannot
block the kernel: it can only block itself, and its peers are bounded by the
queue and channel limits.

## 6. Security

| Threat | Mitigation |
|---|---|
| Guessing another process's handle | Handles are indices into the caller's table; the kernel never accepts a raw channel id |
| Sending to a channel not owned | `endpoint_for_handle` validates the table entry |
| Kernel pointer leak | Messages are copied byte arrays; no handles or kernel addresses are placed in payloads |
| Queue flood | 8 messages × 256 bytes per queue, 32 channels, per-process handle limit |
| Deadlock on peer exit | Exit closes handles and wakes waiters with `EPIPE` |
| Use-after-free of a process | Channel operations run in syscalls with interrupts disabled; the process is only destroyed after its thread is dead and reaped |

Known gaps: no capability layer, no per-service policy, no shared memory for
bulk data, no handle transfer between arbitrary processes, and no timeout.
These are explicit, not hidden.

## 7. Tests

`tools/isolation_test.py` runs:

- `ipc`: create a channel, spawn `ipcprobe`, send `ping`, receive `pong`,
  collect status 0.
- `ipchup`: send on unused handle 15 (expect `-3`), spawn `isoprobe` with a
  bootstrap handle, close the parent's copy, receive (expect `-14` after the
  child exits), collect status 42.

The serial log must contain all of the status lines, `DESKTOP-READY`, and no
kernel panic. The desktop must still accept the `uname` command afterwards.

## 8. Follow-ups

1. Timeouts and non-blocking flags (`ETIMEDOUT`, `EAGAIN`).
2. Bulk transfers through shared memory with refcounted pages.
3. Explicit handle passing between arbitrary processes.
4. Per-process quotas and a channel close-on-exit audit trail.
5. A service manager using bootstrap channels in dependency order.
6. Move the storage filesystem behind a storage service (M4).
