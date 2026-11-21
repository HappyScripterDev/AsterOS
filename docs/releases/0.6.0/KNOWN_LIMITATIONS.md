# Aster OS 0.6.0 - Known Limitations

Status: **not release-complete**. The ACPI and APIC/IOAPIC phases are real and
tested; the remaining platform work listed here is not implemented. No fake
device, sensor, sleep state or acceleration is exposed.

## Implemented and tested

- ACPI RSDP/XSDT validation, FADT, MADT, MCFG and HPET parsing with length and
  checksum checks.
- ACPI S5 shutdown and FADT reset, from Terminal and the desktop confirmation
  dialog.
- `POWER` syscall 47 (0 shutdown, 1 restart); legacy `SHUTDOWN` 28 aliases it.
- Local APIC enable, IOAPIC mapping, ISA IRQ 0/1/12 routing with MADT overrides,
  Local-APIC EOI, PIC masking and PIC fallback.
- Existing desktop, apps, networking, storage, settings and persistence.

## Exact gaps

### Interrupts and drivers

- **No MSI/MSI-X.** The PCI capability list is not parsed for MSI. e1000 and
  AHCI cannot be switched to message-signalled interrupts.
- **e1000 remains polling.** It polls every 10 ms (100 Hz). RX/TX interrupt
  registers are not enabled and the network thread is not woken by an IRQ.
- **AHCI remains polling.** Commands spin up to `WAIT_SPINS = 2_000_000`;
  completion interrupts are not enabled.
- **No APIC timer.** The Local APIC timer is not calibrated and is not the
  scheduler tick. The PIT remains the 1 kHz source, now routed through IOAPIC
  GSI 2. The PIT/PIC fallback still works.
- **No dynamic vector allocator or IPIs.** IDT stubs exist only for 32..47.
  Vectors above 47 and SMP/IPI delivery are absent.
- IRQ counters are proven by one-time "first IRQ" logs, not yet exposed as a
  Monitor/per-CPU counter table.

### USB

- **No xHCI controller, no HID keyboard/mouse, no hotplug.** `tools/run.py` has
  no `--usb` mode yet. PS/2 is the only input path.
- `tools/usb_test.py` is therefore absent; a test without a driver would be a
  placeholder.

### Power and devices

- The ACPI SCI power button is not routed. There is no lid, battery, thermal or
  S3 sleep state; no such rows or buttons exist in the UI.
- **No Device Manager.** Settings has no Devices page, Monitor has no PCI/USB/
  ACPI inventory card, and there is no device-info syscall. PCI enumeration
  remains internal to the e1000/AHCI probes.
- The hardware inventory requested for 0.6.0 (IOAPIC/HPET/MCFG/USB in the UI) is
  only available in the kernel log.
- **No PS/2 wheel.** The mouse uses 3-byte packets, IntelliMouse mode is not
  negotiated and there are no wheel events. Smooth wheel scrolling therefore
  does not exist.

### Tests and release evidence

- `acpi_test.py`, `apic_test.py` and `power_test.py` exist and pass.
- `usb_test.py`, `device_probe.py` and `input_test.py` do not exist because
  their features are absent.
- `TEST_REPORT.md` and `PERFORMANCE.md` describe only measured results; no
  interrupt-latency or USB-latency claim is made.
- The `v0.6.0` tag must not be set while these gaps remain.

## Deferred by scope (0.6.1 / 0.6.2)

Audio, Bluetooth and SMP are 0.6.1. HiDPI, VirtIO-GPU, multi-monitor, Stage
Manager, app-internal damage and the graphics resource cache are 0.6.2. They are
not reopened here.

## Verification snapshot

- `tools/acpi_test.py`: PASS (tables and S5 power-off).
- `tools/apic_test.py`: PASS (LAPIC, IOAPIC routes, timer/keyboard/mouse IRQs).
- `tools/power_test.py`: PASS (Terminal reboot and desktop menu restart).
- The 0.5 graphics, UI, network and storage suites remain green on the APIC path.
