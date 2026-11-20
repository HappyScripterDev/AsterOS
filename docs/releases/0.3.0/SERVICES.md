# Aster OS 0.3.0 - Services

## 1. What shipped

Aster OS now has one real user-space service and a desktop-side supervisor:

| Service | Process | Owns | Protocol |
|---|---|---|---|
| `configd` | child of the desktop, started at boot | `/home/user/SETTINGS.CNF` | `PING`, `GET`, `SET`, `EXIT` over a channel |

The service is a separate ring-3 process. It uses the filesystem syscalls and
IPC; it has no special kernel access.

## 2. Startup and bootstrap

1. The desktop creates a channel (`CHANNEL_CREATE`).
2. It spawns `/home/user/configd.elf` with the second endpoint as the
   bootstrap handle (`SPAWN(path, handle)`).
3. The child receives that endpoint at handle 3 and blocks in `CHANNEL_RECV`.
4. The desktop closes its copy of the child endpoint and sends `PING` with a
   700 ms timeout. `PONG` marks the service Running.

If configd cannot be spawned or does not answer, the desktop logs the failure
and continues with direct settings I/O. A service failure never prevents boot
or locks the user out.

## 3. Settings flow

- **Load**: desktop sends `GET`; configd replies with a validated v1 blob. The
  desktop parses it; invalid values trigger a `SET` with corrected defaults.
- **Save**: desktop serialises the theme and sends `SET\n<blob>`. configd
  validates the size/version/printable content, writes `SETTINGS.NEW`, flushes,
  keeps `SETTINGS.BAK`, renames, flushes and replies `OK`.
- **Corruption**: configd preserves the bad file as `SETTINGS.BAD` and writes
  defaults. A legacy 0.2 file without `version=` is served for migration.
- **Fallback**: if send/recv fails or times out, the desktop performs the same
  versioned atomic write itself and retries the service on later changes.

## 4. Failure detection and restart

The desktop supervisor runs every 2 seconds:

- `PROC_LIST` is scanned for the service pid.
- Missing or zombie (`state == 1`) → close the channel, `WAIT` to reap, log,
  increment the restart counter, start a new configd.
- `svcfail` in the Terminal performs a deliberate graceful restart: `EXIT`,
  `BYE`, reap, respawn. The service test verifies the pid changes (2 → 3) and
  that a subsequent accent change is still persisted.

Logs are emitted through the kernel `LOG` syscall and appear in the Monitor
Logs tab and serial console.

## 5. IPC timeout

`CHANNEL_RECV` now accepts a timeout in milliseconds (4th argument). The
scheduler blocks with an uptime deadline; the timer wakes the thread, stale
waiters are removed, and `ETIMEDOUT` is returned if no message arrived. This is
what makes a hung service observable instead of freezing the shell forever.

## 6. Limits

- No generic service manager, dependency graph, discovery protocol or
  capability model yet. The desktop is the supervisor and only configd is
  managed.
- There is no `KILL`; a hung (rather than exited) service cannot be forcibly
  terminated. The desktop falls back to direct I/O in that case.
- No handle passing beyond `SPAWN` bootstrap, and no shared memory.
- configd accepts structurally valid v1 blobs from its parent; semantic
  validation lives in the desktop, which immediately rewrites corrected values.
- Service permissions are the same as any user process: world-readable
  filesystem, no per-service namespace.

These limits are deliberate for the first service milestone and are tracked in
`KNOWN_LIMITATIONS.md`.
