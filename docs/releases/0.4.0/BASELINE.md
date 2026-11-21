# Aster OS 0.4.0 - Baseline Audit

Date: 2026-11-21
Branch: `milestone/aster-0.4.0`
Baseline tag: `v0.3.0-baseline` (commit `ba777a3`)
Previous release: `v0.3.0`

This document records the **verified** state at the start of 0.4.0. Every claim
was checked against the tree and by running the test suite, not inferred from
older roadmaps.

## 1. Verification runs

| Check | Result |
|---|---|
| `python tools/build.py` | PASS: bootloader, kernel, desktop, 512 MiB boot image, 512 MiB data image |
| `python tools/test.py` | PASS: host tests, build, boot smoke, no faults |
| `python tools/ui_test.py` | PASS |
| `python tools/app_probe.py`, `pointer_test.py`, `wm_probe.py` | PASS |
| `python tools/persistence_test.py` | PASS: guest write, host FAT verifier, reboot, read-back |
| `python tools/storage_probe.py` | PASS: capacity, missing disk, corrupt volume |
| `python tools/isolation_test.py` | PASS: status 42, fault -11, IPC pong, forged handle -3, hangup -14 |
| `python tools/settings_test.py`, `service_test.py`, `files_probe.py`, `fs_probe.py` | PASS |
| Benchmark | first frame 4.9 s in the recorded 0.3.0 report; 446 MiB free at idle |

## 2. What exists at the baseline

### Boot and kernel

- Custom UEFI loader (`Aster Boot`) with GPT/FAT32 ESP, ELF64 kernel loading,
  higher-half page tables and `ExitBootServices`.
- q35 reference machine: 1 vCPU, 512 MiB RAM, OVMF, std VGA 1280x800, PS/2,
  two IDE disks (boot + persistent data), PIT at 1000 Hz.
- PIC only. There is no APIC, IOAPIC, MSI or PCI IRQ routing. AHCI and PS/2
  work; AHCI polls.
- 4-level paging per process, NX, W^X, user-copy validation, private PML4.
- Preemptive single-CPU scheduler, ring-3 processes, `int 0x80` syscall gate.
- Syscall ABI: numbers 1-36 assigned; 35 implemented. `SET_THEME` (27) returns
  `ENOSYS` because the appearance is fixed to light.

### Storage and services

- Block registry with capacity, bounds, flush and I/O counters.
- AHCI polling driver, GPT parser, FAT32 at `/home/user`, ramfs root.
- Versioned `version=1` settings, validation, `SETTINGS.BAK` / `SETTINGS.BAD`.
- `configd` service owns settings over IPC; the desktop supervises and restarts
  it and falls back to direct I/O.
- Files app operations, Monitor storage/log pages.

### Desktop

- Aster Desktop in ring 3: menu bar, Spotlight, dock, 9 apps, window manager,
  Control Centre, notifications, virtual desktops.
- The top bar always draws a Wi-Fi glyph with the tooltip "No network adapter".
- The Control Centre shows static "Wi-Fi / No adapter" and "Bluetooth / No
  adapter" tiles.
- Settings has a Network section that says "No network stack" and lists
  Ethernet/IPv4/DHCP/DNS/TCP/UDP as not implemented.
- Monitor has no network card.

## 3. Hardware relevant to this milestone

The default q35 configuration already exposes one network controller:

| Property | Value |
|---|---|
| PCI address | bus 0, device 1, function 0 |
| Vendor / device | `8086:10d3` (e1000 class; QEMU's 82540EM) |
| BAR0 | MMIO `0x81060000`, 128 KiB |
| BAR1 | MMIO `0x81040000` |
| BAR2 | I/O `0x6060` |
| IRQ line | 10 (not routed; no IOAPIC) |

`kernel/src/drivers/pci.rs` can read configuration space and locate devices by
vendor/device. `vmm::map_kernel_mmio` can map device BARs. The direct map only
covers RAM and the framebuffer.

## 4. Constraints and decisions for 0.4.0

1. **One NIC only**: implement the e1000 (82540EM) completely before adding any
   other controller. VirtIO-net, e1000e, igb and RTL8139 are out of scope.
2. **No interrupts for the NIC**: because no PCI IRQ routing or APIC exists,
   the driver uses a kernel network thread polling at 100 Hz and synchronous TX
   completion. This is documented, measured and stated in the UI limitations.
3. **Real QEMU networking**: tests use QEMU user-mode networking (SLIRP) with
   an explicit `e1000` device. Host-side TCP, UDP and DNS servers provide real
   peers. No status is faked if the device is absent.
4. **Additive ABI**: syscalls 1-36 keep their numbers. Networking uses 37-46.
5. **No IPv6 or TLS in this milestone**: only architecture notes, no partial
   or fake protocol code.

## 5. Existing limitations preserved

- Built-in apps share the desktop address space; only spawned processes are
  isolated.
- FAT32 uses 8.3 names, no journal, no permissions.
- No `KILL`, no per-process quotas, no ASLR/SMEP/SMAP/UMIP.
- No generic service manager; `configd` is the only managed service.
