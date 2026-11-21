# Aster OS 0.6.0 - Baseline Audit

Date: 2026-11-21
Branch: `milestone/aster-0.6.0-platform`
Base: `milestone/aster-0.5.0-gpu` tip `b79e6a5`; annotated `v0.5.0` untouched.

## 1. Build and test baseline

| Check | Result |
|---|---|
| `python tools/build.py` | PASS, 512 MiB image verified |
| `python tools/test.py` | PASS |
| `python tools/ui_test.py` | PASS |
| `python tools/pointer_test.py` | PASS |
| `python tools/wm_probe.py` | PASS |
| `python tools/settings_test.py` | PASS |
| `python tools/app_probe.py` | PASS |
| `python tools/net_test.py` | PASS |
| `python tools/persistence_test.py` | PASS |

Measured boot on this host (QEMU TCG, q35, 1 vCPU, 512 MiB):

- kernel/network ready: ~0.63 s to first e1000 RX and DHCP bind;
- desktop first frame: ~0.70 s;
- first frame: 1,024,000 dirty px, 9 ms draw, 1 ms present;
- idle desktop: frames skipped when nothing is dirty.

## 2. Interrupt and timer baseline

- 8259 PIC only. Vectors: 32 timer, 33 keyboard, 44 mouse, 0x80 syscall.
- PIT channel 0 at ~1 kHz; `time::uptime_ms()` counts PIT IRQs. Heavy guest
  load can lose PIT ticks under TCG, so guest time can lag wall time.
- No Local APIC, IOAPIC, MSI/MSI-X, APIC timer or dynamic vector allocator.
- EOI goes to the PIC. There is no active interrupt backend to report.
- The scheduler is preemptive on the single CPU via the PIT.

## 3. Driver baseline

| Driver | Path | Baseline |
|---|---|---|
| e1000 | PCI 8086:100E, MMIO BAR0 | 32 RX / 8 TX descriptors, RX/TX interrupts disabled, network thread polls every 10 ms (100 Hz). DHCP/ICMP/TCP/UDP/DNS pass. First RX 590 bytes at ~0.63 s. |
| AHCI | PCI class 0x0106 | One command slot, polling with `WAIT_SPINS = 2_000_000`, single-sector DMA read/write, FLUSH CACHE EXT. Persistence passes. |
| PS/2 | i8042 | Keyboard set-1 scancodes; mouse 3-byte packets, absolute pointer. No IntelliMouse wheel, no typematic repeat. |
| PCI | mechanism #1 | Enumerates bus 0, finds e1000 by vendor/device and AHCI by class. No capability parser, no MSI/MSI-X, no full inventory surface. |
| Storage | AHCI + FAT32 | Mounts the data disk at `/home/user`; 8.3 names, no journal. |

## 4. Missing platform pieces

- ACPI: only `BootInfo.rsdp_address` is captured. No RSDP validation, XSDT/RSDT
  walk, FADT, MADT, MCFG or HPET parsing. No S5 shutdown, no reset, no SCI.
- Power: no `POWER` syscall, no Terminal `shutdown`/`reboot`, no desktop power
  menu. The only shutdown path is the QEMU debug-exit port used by probes.
- USB: no xHCI controller, no USB HID, no hotplug. The default QEMU run has no
  USB controller attached.
- Devices: no Device Manager, no hardware inventory, no IRQ/MSI display.
- Input: no mouse wheel and no smooth wheel scrolling.
- Explicitly deferred to 0.6.1/0.6.2 and not reopened here: audio, Bluetooth,
  SMP, HiDPI, VirtIO-GPU, multi-monitor, Stage Manager, app-internal damage and
  the resource cache.

## 5. Device inventory (default QEMU q35 run)

| Device | Address | Driver | Notes |
|---|---|---|---|
| Intel 82540EM e1000 | PCI 00:03.0 | `drivers/e1000.rs` | polling |
| ICH9 AHCI | PCI 00:04.0 (class 0x0106) | `drivers/ahci.rs` | polling |
| QEMU stdvga | PCI 00:01.0 | GOP framebuffer | software composition |
| i8042 PS/2 | ISA | `drivers/ps2.rs` | keyboard/mouse |
| PIT 8254 | ISA | `time.rs` | 1 kHz tick |
| 8259 PIC | ISA | `arch/pic.rs` | master/slave, EOI |

No xHCI, HDA, battery or thermal device is claimed. A `qemu-xhci` run will be
added for USB tests.

## 6. Documentation corrections

The Phase 0 docs now match the 0.5 code:

- live glass samples the composited backbuffer with downsample and two box-blur
  passes; it is no longer described as precomputed wallpaper blur;
- per-window surfaces and window-level damage are described as shipped;
- SSE2 opaque row fill/copy, fullscreen, Alt-Tab thumbnails, desktop edge-drop
  and four workspaces are recorded;
- deferred 0.5 gaps are assigned to 0.6.2 in `docs/ROADMAP.md`.

## 7. Gate

Build green, existing tests green, baseline measured, docs corrected. Proceed to
the ACPI core on this branch.
