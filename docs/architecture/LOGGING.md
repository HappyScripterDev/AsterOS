# Kernel and Service Logging

## 1. Current implementation

`kernel/src/logging.rs` writes each message to COM1 and to a 64 KiB in-memory
ring. The format is:

```
[   12.345] info: storage: FAT32 'ASTERDATA' mounted, 130552 clusters
```

- `[seconds.milliseconds]` is the in-guest uptime from the PIT.
- `info`, `warn`, `error`, `debug` are the severity levels.
- The subsystem is a prefix convention inside the message (`storage:`,
  `mount:`, `service:`, `settings:`, `files:`, `configd:`). It is deliberately
  human-readable rather than a separate binary field in this milestone.
- Macros: `info!`, `warn!`, `error!`, `debug!` in kernel code; user space uses
  the `LOG` syscall, which prefixes `[user] ` and appends a newline.
- Writers are serialized by an interrupt-safe lock. A full ring overwrites the
  oldest bytes; logging never blocks on a reader and never allocates beyond the
  message formatting already in use.

## 2. Reading logs

`LOG_READ` (syscall 35) copies the newest `min(capacity, 16 KiB)` bytes of the
ring to a validated user buffer. It is non-blocking. Monitor uses a 3 KiB
buffer once per refresh and renders the last lines in its Logs tab; it does not
allocate per frame.

The serial console remains the authoritative boot log. The ring exists for
in-guest diagnostics and the Monitor view.

## 3. What is logged

- boot contract, GDT/IDT/PIC/PIT, PMM/VMM/heap, self-tests;
- block devices, GPT partitions, FAT mount and self-tests;
- mount table attachment and route self-test;
- process spawn, exit, wait, faults and IPC handle errors;
- settings load/migration/corruption and configd start/save/exit;
- Files create/rename/delete/copy operations initiated by the user.

## 4. Limits

- The ring is volatile and 64 KiB; there is no rotation to disk.
- There is no separate structured field for subsystem, pid or severity beyond
  the text prefix; parsing is textual.
- User-space logs are not rate-limited or quota-limited. A process can fill the
  ring with `LOG` calls; the syscall caps each call at 4 KiB.
- No log service or remote sink yet. A future logging service can consume
  `LOG_READ` over IPC and persist to `/home/user/LOGS`.

## 5. Follow-ups

1. Structured records (timestamp, level, subsystem, pid) behind a versioned
   ABI without breaking the text console.
2. A ring service that drains the kernel buffer and writes `/home/user/LOGS`
   with rotation.
3. Per-process log quotas and an audit trail for denied operations.
