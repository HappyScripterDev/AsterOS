# Aster OS 0.6.0 - Test Report

Date: 2026-11-21
Branch: `milestone/aster-0.6.0-platform`
Backend under test: Local APIC + IOAPIC (PIC fallback compiled and reachable).

## Results

| Command | Result |
|---|---|
| `python tools/build.py` | PASS, image verified |
| `python tools/test.py` | PASS |
| `python tools/net_test.py` | PASS (DHCP, ICMP, TCP, UDP, DNS) |
| `python tools/persistence_test.py` | PASS |
| `python tools/ui_test.py` | PASS |
| `python tools/pointer_test.py` | PASS |
| `python tools/wm_probe.py` | PASS |
| `python tools/settings_test.py` | PASS |
| `python tools/app_probe.py` | PASS |
| `python tools/acpi_test.py` | PASS (tables and S5 power-off) |
| `python tools/apic_test.py` | PASS (LAPIC, IOAPIC, three ISA routes, first IRQs) |
| `python tools/power_test.py` | PASS (Terminal reboot, desktop confirmation restart) |

Not present because the feature is absent: `tools/usb_test.py`,
`tools/device_probe.py`, `tools/input_test.py`. No placeholders were added.

## ACPI evidence (q35)

- RSDP revision 2, XSDT 84 bytes.
- FADT 244 bytes: PM1a `0x604`, PM1b `0`, SLP_TYPa `0`, reset I/O `0xCF9`
  value `0xF`, SCI IRQ 9.
- MADT 120 bytes: LAPIC `0xFEE00000`, 1 CPU, 1 IOAPIC at `0xFEC00000`, 5 ISOs.
- MCFG base `0xE0000000`, segment 0, buses 0..255.
- HPET base `0xFED00000`, 3 comparators.

## Interrupt evidence

- `apic: active, PIC masked, EOI -> LAPIC`.
- IRQ 0 -> GSI 2 -> vector 32; IRQ 1 -> GSI 1 -> vector 33; IRQ 12 -> GSI 12 ->
  vector 44.
- First real timer, keyboard and mouse IRQs logged. No interrupt storm was
  observed and the desktop reached its first frame.

## Power evidence

- Terminal `shutdown` reaches `acpi: S5 shutdown` and QEMU powers off.
- Terminal `reboot` and the desktop Restart confirmation both reach
  `acpi: reset requested` and QEMU exits under `-no-reboot`.

## Not verified

- MSI/MSI-X, APIC timer calibration, interrupt-driven e1000/AHCI, xHCI/USB,
  Device Manager, hardware inventory and PS/2 wheel. These are documented gaps,
  not passes.
- No interrupt-latency or USB-latency measurement is claimed.
