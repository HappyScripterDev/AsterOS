# Aster OS 0.3.0 - Test Report

Date: 2026-11-20
Branch: `milestone/aster-0.3.0`
Baseline: `v0.2.0-baseline` (`68f499a`)

All results below were executed in this environment, not inferred.

## 1. Regression suite

| Check | Result |
|---|---|
| `python tools/test.py` | PASS: host image tests, build, boot smoke, storage markers, no faults |
| `python tools/ui_test.py` | PASS: desktop, Terminal, Settings, Files, Spotlight |
| `python tools/app_probe.py` | PASS: all nine applications captured |
| `python tools/pointer_test.py` | PASS: dock, notifications, Control Centre, brightness |
| `python tools/wm_probe.py` | PASS: drag and virtual desktops |
| `python tools/storage_probe.py` | PASS: storage page, missing disk, corrupt volume |
| `python tools/persistence_test.py` | PASS: write, reboot, read, host FAT verification |
| `python tools/isolation_test.py` | PASS: spawn 42, fault -11, IPC pong, forged handle -3, hangup -14, bad syscalls -5 |
| `python tools/fs_probe.py` | PASS: 64 KiB write/read, rename, delete, sync |

## 2. New 0.3.0 tests

### Storage and mounts

- `storage: block self-test passed (bounds + flush)` at boot: reads LBA 0,
  rejects `LBA >= capacity`, and executes FLUSH CACHE EXT.
- `storage: mount self-test passed`: root resolves to ramfs; `/home/user` and
  nested paths resolve to the FAT32 mount with the longest prefix.
- `STORAGE_DETAILS` is displayed by Settings: device 1, FAT32 (ASTERDATA),
  510.0 MiB capacity, used/free, partition LBA 2048, real I/O counters.

### Filesystem correctness

`tools/fs_probe.py` runs the Terminal `fstest` command:

```
fstest: write 64 KiB 19 ms (size 65536), read 17 ms (65536 B, first bad -1), rename ok, delete ok
```

This test found and verified a real bug: multi-sector FAT writes previously
repeated the first 512-byte sector. The fix is in `write_range`; the test now
compares the entire 64 KiB pattern, not just the first and last byte.

### Settings reliability

`tools/settings_test.py` builds three fresh FAT32 volumes and boots each:

| Case | Result |
|---|---|
| Structurally corrupt file | Preserved as `SETTINGS.BAD`; defaults written as v1 |
| v1 with invalid values (`accent=notacolour`, `animations=99`, `wallpaper=99`, `reduce_motion=7`, `brightness=9`) | Corrected to validated defaults, then persisted |
| Valid custom v1 (green, 0.75x, wallpaper 2, reduce motion, 0.90) | Served unchanged across boot |

### Services

`tools/service_test.py`:

- `services` reports `configd: running (pid 2)`.
- `accent green` produces `configd: settings saved`.
- `svcfail` sends `EXIT`, the desktop reaps the child and restarts configd;
  the next `services` reports pid 3.
- `accent orange` is saved through the restarted service.

### Files application

`tools/files_probe.py` uses only real keyboard operations on a fresh volume:

- Ctrl+N creates `NEWDIR`.
- F2 + typing renames it to `TESTDIR`.
- Ctrl+C, navigate to Documents, Ctrl+V copies it.
- Delete twice removes the original.
- Ctrl+F filters to `WELCOME.TXT`; Ctrl+I shows its information.

Serial evidence: `files: created NEWDIR`, `files: renamed NEWDIR to testdir`,
`files: copied TESTDIR to TESTDIR`, `files: deleted TESTDIR`.

## 3. UI verification

- Files: sidebar (Home, Documents, Downloads, Desktop, Notes, System, Logs,
  Root), breadcrumbs, toolbar, list with sizes, search, information modal.
- Settings > Storage: mount, filesystem, device, capacity, used, free,
  partition, I/O since boot, durability, settings format.
- Monitor: six cards including Storage and Disk free; Processes/Logs tabs;
  log view renders the kernel ring without blocking.
- Services screenshot: pid changes from 2 to 3 and the accent prompt changes to
  orange after the restart.

## 4. QEMU configuration

q35, one vCPU, 512 MiB, OVMF, std VGA 1280x800, PS/2, serial file, IDE disk 0
(boot) and IDE disk 1 (data). Tests use per-run QMP ports and, where
determinism matters, freshly built FAT32 volumes from `tools/seeds.py`.

## 5. Not tested

- Physical hardware, NVMe, USB, removal while mounted.
- Power loss during a multi-sector write (no journal).
- Concurrent writers (single-user, no file locking).
- Settings service behaviour when it hangs rather than exits (no KILL yet).
