# System Services

## 1. What exists today

Aster OS has no separate init daemon yet. The kernel starts the desktop as
PID 1; the desktop supervises `configd`, the first user-space service, over a
bootstrap channel. Kernel-resident services remain syscalls. A generic service
manager with dependencies, discovery and capabilities is the next step; the
current supervisor only starts, detects, reaps and restarts configd.

| Service | Location | Interface | State |
|---|---|---|---|
| Serial console / kernel log | `logging.rs`, `arch/serial.rs` | kernel internal, 64 KiB ring buffer | Implemented |
| Framebuffer | GOP info in `BootInfo`, `syscall::FB_INFO/FB_MAP` | user mapping + info | Implemented |
| Input | `drivers/ps2.rs`, `syscall::INPUT_POLL` | event queue | Implemented |
| Time | PIT ticks (`TIME_MS`), CMOS RTC (`RTC`) | syscalls | Implemented |
| Memory | PMM/VMM/heap, `VM_ALLOC`, `SYSTEM_INFO` | syscalls | Implemented |
| Filesystem | `fs.rs` ramfs | `OPEN/READ/WRITE_FD/...` | Implemented (volatile) |
| Process information | `sched::list_processes`, `PROC_LIST` | syscall | Implemented |
| Process creation | `SPAWN`/`WAIT`, private PML4 | syscall | Implemented |
| IPC | `ipc.rs`, `CHANNEL_*` | syscall | Implemented (channels, handles, blocking) |
| Persistent home | AHCI + GPT + FAT32 at `/home/user` | VFS syscalls | Implemented |
| Settings service | `configd` (ring 3) | IPC `PING`/`GET`/`SET`/`EXIT` | Implemented |
| Service supervisor | desktop `services.rs` | spawn, PROC_LIST, WAIT, restart | Implemented (configd only) |
| Session / shell / WM | ring-3 `asterdesk` | owns the screen | Implemented |
| Built-in apps | `apps.rs` | in-process modules of the desktop | Implemented |

Not present: storage service, network service, audio service, user/identity
service, policy/permission broker, package service, update service, device
manager, power manager, log service, notification daemon, clipboard service.

## 2. Boot-time startup sequence

```
kernel init
  -> ramfs seeded (/etc, /system, ...)
  -> AHCI/GPT/FAT32 mounted at /home/user (if the data disk exists)
  -> scheduler idle thread
  -> load /DESK ELF as PID 1
  -> scheduler start
  -> desktop maps framebuffer, allocates its heap, builds wallpaper
  -> desktop starts configd, pings it, then loads settings over IPC
  -> configd falls back to direct /home/user/SETTINGS.CNF I/O if unavailable
  -> desktop shows the shell (no login; single implicit user)
  -> desktop may SPAWN children and exchange channel messages
```

If the desktop module is missing or fails to load, the kernel logs the error,
stays alive on the idle thread, and does not fake a desktop.

## 3. Service boundaries

- **Kernel services** are exposed only through syscalls with validated user
  pointers. The desktop has no hardware access beyond the mapped framebuffer.
- **Spawned processes** get private address spaces and cannot read each
  other's memory. They communicate through kernel channels with per-process
  handles.
- **Desktop apps** are still modules of the desktop process. They share its
  address space, so a crash in one app currently takes down the shell. Moving
  them onto the process and IPC foundations is the next application milestone.
- **Filesystem access** is global: there is no per-user or per-app mount
  namespace, ACL, or capability check yet.

## 4. Planned service model

Order of work, with dependencies:

1. ~~`SPAWN`/`WAIT` syscalls and a minimal init process.~~ Done.
2. ~~Channel IPC plus `CHANNEL_*` syscalls.~~ Done.
3. ~~A first service and supervisor.~~ Done for configd; a generic manager
   with dependencies, discovery and restart policy is next.
4. Storage service over IPC on top of the existing AHCI/FAT32 stack.
5. Identity service: user database, login, per-process credentials.
6. Policy service: capability checks for filesystem, network, devices.
7. Network service: Ethernet -> IPv4 -> DHCP/DNS -> TCP/UDP -> socket API.
8. Update/package service: signed manifests, A/B or snapshot rollback.

Acceptance criteria for the service manager milestone:

- Services can be started, stopped, and restarted without rebooting.
- A failed service cannot take down the kernel or the desktop.
- Each service has an explicit capability set; denied access returns an error.
- Logs from services are collected with a timestamp and service name.
- A boot with one service disabled still reaches the desktop.

## 5. API stability

`shared/aster-abi` is versioned (`SYS_ABI_VERSION`, `BOOTINFO_VERSION`). Adding a
syscall does not bump the version; changing an existing layout or number does.
The bootloader, kernel, and desktop are built together in this repository, but
the ABI is treated as external because third-party applications will depend on
it.

## 6. Current limitations

- Single user (`user`), no passwords, no lock screen, no privilege escalation.
- Built-in applications still share the desktop address space; only spawned
  children (configd and the probes) have private address spaces.
- One managed service and one supervisor; no generic service manager,
  dependency graph, discovery or policies.
- No device manager or hotplug; PS/2 and GOP are initialised directly.
- System files remain in ramfs; `/home/user` is persistent on the data disk,
  with 8.3 names and no journal.
- No `KILL`: a hung service cannot be forcibly restarted; the desktop falls
  back to direct settings I/O.
