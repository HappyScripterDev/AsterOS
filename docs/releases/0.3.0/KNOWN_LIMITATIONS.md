# Aster OS 0.3.0 - Known Limitations

This is the honest list of things Aster OS 0.3.0 does **not** do, or does with
weaker guarantees than a production OS.

## Storage and filesystem

- FAT32 uses 8.3 names. `settings.conf`, long user names and lowercase display
  names are not supported; the UI shows uppercase names.
- No journal, checksums or transaction. A power loss during a write can
  truncate a file or leave a leaked cluster. `SETTINGS.BAK` protects the
  settings file only.
- One data disk, one FAT32 mount at `/home/user`, no unmount, hotplug or
  removable media.
- AHCI polling, one 512-byte sector per command, no DMA cache. 64 KiB write is
  ~19 ms, read ~17 ms under QEMU TCG.
- No file permissions or ownership. The volume is world-readable/writable for
  the single implicit user.
- Rename is same-directory only. Cross-directory move is copy+delete in Files.
- No file locking; concurrent writers are undefined (the desktop is one process
  today).
- The boot ESP is read-only from the kernel; OVMF may still write `NVVARS` to
  it during firmware boot.

## Services and processes

- One managed service (`configd`). No generic service manager, dependency
  resolution, discovery, per-service capabilities or restart policy language.
- No `KILL`. A service that hangs instead of exiting cannot be forcibly
  terminated; the desktop falls back to direct settings I/O.
- IPC messages are 256 bytes inline; no shared memory, arbitrary handle
  passing or bulk path.
- IPC has a receive timeout but no send timeout or non-blocking mode.
- No per-process memory quotas. A malicious child can exhaust physical memory.
- No kernel/user guard pages, ASLR, SMEP, SMAP or UMIP.
- No SMP; one CPU, cooperative kernel threads.

## Desktop and applications

- The nine built-in applications still share the desktop address space. Only
  spawned children (probes, configd) are isolated.
- Any process that calls `FB_MAP` can write the framebuffer; no display
  capability.
- Any process can call `SHUTDOWN`; no capability check.
- Files supports list view only, no grid, drag-and-drop, multiple selection,
  tabs or trash. Search filters the current directory, not the whole volume.
- Copy/move is recursive but depth-limited to 8 and has no progress UI.
- Settings exposes only implemented options: accent, wallpaper, animation
  speed, reduce motion, brightness. Dock/notification preferences do not exist
  as controls, so they are not persisted.
- Notifications are in-memory; there is no notification service or history
  across reboot.

## Logging

- Kernel ring is 64 KiB and volatile. Logs are exposed through `LOG_READ` to
  Monitor but are not written to disk or rotated.
- Subsystem identity is a message prefix convention (`storage:`, `service:`),
  not a structured field.

## Performance

- Full-frame software composition every frame; no damage tracking.
- 48 MiB desktop heap is mapped eagerly.
- Brightness below 100% adds a full-surface blend (~30 ms/frame under TCG).
- Five boot modules (desktop, three probes, configd) are copied by the loader,
  adding ~0.1-0.2 s to boot.

## Not implemented at all

Networking, audio, USB, NVMe, installer, package manager, update service,
multi-user login, permissions/ACLs, SMP, GPU acceleration, suspend/resume,
long file names, journaling.

No feature in the UI claims any of these work. Settings explicitly reports
missing network, Bluetooth and audio hardware/drivers.
