# Aster OS 0.6.0 - Known Limitations

Status: **not release-complete**. The platform, interrupt, input and device
inventory work below is real and tested. USB xHCI is still absent, so the
`v0.6.0` tag must not be set.

## Implemented and tested

- ACPI RSDP/XSDT, FADT, MADT, MCFG, HPET; S5 shutdown and FADT reset.
- `POWER` 47 and `DEVICES` 48 syscalls; legacy `SHUTDOWN` 28.
- Local APIC, IOAPIC ISA routing and EOI; PIC fallback.
- APIC timer calibrated against the HPET (100 MHz in QEMU), 1 kHz on vector 63;
  the PIT route is masked only after successful calibration.
- Dynamic vectors 48-62 with a registry and IRQ counters.
- AHCI MSI completion interrupts (vector 48); e1000 RX/TX interrupts through
  IOAPIC INTx (vector 49) because QEMU's 82540EM exposes no MSI capability.
  Both drivers keep bounded polling/timeout fallbacks.
- PS/2 IntelliMouse wheel detection, 4-byte packets, wheel events; Files
  scrolls on the wheel.
- Settings > Devices and Terminal `devices` show the real PCI inventory with
  vendor/device/class/IRQ and driver state.

## Remaining gaps

- **USB xHCI, HID keyboard/mouse and hotplug are not implemented.**
  `tools/run.py` has no `--usb` mode, and `tools/usb_test.py` is absent rather
  than a placeholder. This is the main reason the release is incomplete.
- **Monitor has no device card.** Settings > Devices and Terminal `devices` are
  the inventory surfaces; the Monitor app does not add a PCI/USB card.
- **Wheel scrolling is wired to Files only.** Text has smooth Page Up/Down
  scrolling but no wheel handler; Settings content is not scrollable by wheel.
- **MSI-X is not implemented.** Only the MSI capability (one vector) is parsed; no MSI-X table/PBA programming exists.
- **e1000 MSI path is untested.** The PCI MSI parser exists and AHCI MSI is
  verified; QEMU's `e1000` device has no MSI capability (`status=0`, cap list
  empty), so the tested path is IOAPIC INTx. No MSI success is claimed for
  e1000.
- **AHCI/e1000 still have polling fallbacks** for platforms without MSI/INTx and
  for timeout recovery. The active path is logged (`MSI`, `PCI INTx`, or
  `polling`).
- **No ACPI SCI power button, lid, battery, thermal or S3 sleep.** No such UI
  exists.
- **No USB mass storage, audio, Bluetooth, SMP or GPU.** Deferred by scope to
  0.6.1/0.6.2.

## Test coverage

`acpi_test.py`, `apic_test.py`, `power_test.py`, `input_test.py` and
`device_probe.py` pass together with the existing suite. `usb_test.py` is
absent because there is no USB driver. No interrupt-latency or USB-latency
number is claimed.
