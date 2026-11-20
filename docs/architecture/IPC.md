# Inter-Process Communication (IPC)

## 1. Status: designed, not implemented

The ABI reserves `CHANNEL_CREATE`, `CHANNEL_SEND`, and `CHANNEL_RECV`
(`shared/aster-abi/src/lib.rs`). The kernel currently returns `ENOSYS` for all
three. There is **no working IPC** in this build, and no UI or documentation
claims otherwise.

Today, data exchange happens through:

- the global ramfs (all processes share it; no namespaces or permissions),
- the framebuffer mapping (owned by the desktop; not multiplexed),
- copied syscall buffers (request/response only, no asynchronous messages).

This is sufficient for a single ring-3 shell with in-process apps, but it is
not sufficient for services or sandboxed applications.

## 2. Requirements for the first IPC implementation

Functional:

- Bidirectional message channels between two processes.
- Fixed-size small messages (inline) and a path for bulk data.
- Blocking send/receive with timeouts; wake-up via the scheduler.
- Close/hangup semantics that unblock waiters with a distinct error.
- No message loss under backpressure; bounded queues with a defined overflow
  policy (block by default).

Security:

- A process may only send to a channel it holds a handle for.
- Handles are process-local indices, not global pointers or pids.
- The kernel validates every user buffer before reading or writing.
- No kernel pointers, capabilities, or file descriptors are leaked in message
  payloads; handle transfer is explicit and controlled.
- Resource limits per process to prevent a channel flood from exhausting the
  kernel heap.

Performance:

- Inline messages avoid allocation on the fast path.
- Optional shared-memory handles for large payloads once VMM supports shared
  mappings with explicit sharing (not implemented).
- Copying a 64-byte message should be O(1) and not take a global lock that
  blocks unrelated channels for long.

## 3. Proposed design

### Handles

Each `Process` gets a channel table (like the fd table). `CHANNEL_CREATE`
returns a handle pair: a send end and a receive end, each an index. A channel is
a kernel object with two ring buffers (or one buffer with two cursors) and a
reference count per direction. `CHANNEL_SEND(handle, buffer, length)` enqueues;
`CHANNEL_RECV(handle, buffer, capacity)` dequeues.

### Message layout

```
struct MessageHeader {
    u32 length;        // payload bytes
    u32 flags;         // e.g. CLOSE, ERROR, FUTURE_HANDLE_TRANSFER
    u64 sender_pid;    // filled by the kernel
    u64 sequence;      // per-channel monotonic counter
}
```

Initial payload limit: 256 bytes inline. Larger data must use ramfs or a future
shared-memory object. This keeps the first implementation auditable and avoids
a page-granular allocator in the channel path.

### Blocking and fairness

- `send` blocks when the channel is full, with an optional timeout in ms.
- `recv` blocks when empty; timeout and close both return defined errors.
- Waiters are queued per direction and woken in FIFO order by the scheduler.
- Interrupts are disabled only while manipulating channel state, never while
  copying a user buffer.

### Transfer of handles

Not in v1. When added, a handle transfer must be an explicit flag plus a
kernel-mediated table insertion on the receiver, never a raw index passed as
data, otherwise a process could guess another process's handles.

### Bootstrapping

When `SPAWN` exists, the parent creates a channel and places the child's send
handle in the child's initial handle table at a known index (e.g. 3). The child
receives a bootstrap channel on startup. A service manager uses this to talk to
each service.

## 4. Alternatives considered

| Option | Pros | Cons | Decision |
|---|---|---|---|
| Shared memory only | Fastest, simple kernel | No synchronisation, hard to sandbox, pointer sharing | Rejected for control messages; future bulk path |
| Unix-style sockets | Familiar, flexible | Large surface, filesystem namespace coupling | Rejected for v1 |
| Microkernel ports (seL4-style) | Strong isolation, capability-based | Requires capability system and endpoint objects; more work | Long-term direction, incremental |
| Syscall-mediated mailbox | Small kernel surface, easy validation | Copies messages, less flexible | Selected for v1 |

## 5. Security threats and mitigations

| Threat | Mitigation |
|---|---|
| Fake sender pid | Kernel fills `sender_pid` from the calling thread; payload cannot override |
| Cross-channel handle guessing | Handle indices are validated against the caller's table |
| Kernel heap exhaustion via messages | Per-process channel and byte quotas; `ENOSPC` on overflow |
| Deadlock between two services | Timeouts on both ends; close unblocks waiters; no kernel lock held during copy |
| Message contents executed as pointers | Messages are data only; no kernel pointer dereference |
| Starvation | Fair FIFO wake-ups, per-channel quotas, scheduler quantum for user threads |

## 6. Test plan

1. Kernel unit tests for the channel ring buffer and quota accounting.
2. Two ring-3 test processes once `SPAWN` exists: ping-pong 10,000 messages,
   verify sequence and payload.
3. Backpressure test: sender fills the queue, receiver drains; assert no message
   loss and no kernel panic.
4. Timeout test: `recv` with no sender returns `ETIMEDOUT`; close returns
   `EPIPE`.
5. Security test: a process attempts to send on a guessed handle; expect
   `EBADF`.
6. Stress: 16 channels × 64 KiB/s for 30 s; check kernel heap and PMM stability.

## 7. Follow-ups

- Decide whether `CHANNEL_CREATE` is one call returning two handles or two
  calls (`channel_create` + `channel_pair`) for clarity.
- Define how channels are inherited across `SPAWN` (currently: no inheritance).
- Align error codes with `aster_abi::errno` (`EPIPE`, `ETIMEDOUT`, `ENOSPC`).
- Expose channel handles through the planned capability system instead of
  plain per-process indices.
