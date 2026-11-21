# ACPI

Aster OS 0.6.0 parses the ACPI fixed tables needed for platform discovery and
power. There is no AML interpreter: DSDT/SSDT bodies are not executed, only the
fixed tables listed here are used.

## 1. Discovery

- The UEFI loader writes `BootInfo.rsdp_address`. The kernel validates the
  RSDP signature and its 20-byte checksum, and for revision 2+ the extended
  36-byte checksum.
- If the boot pointer is invalid, the BIOS/EBDA range `0xE0000..0x100000` is
  scanned in 16-byte steps for a valid RSDP.
- XSDT is preferred for revision 2+; RSDT is the fallback.
- Every system description table is bounds-checked (`36..=1 MiB`) and its
  checksum is verified before any field is read. Invalid tables are logged and
  skipped.
- Tables are read through the bootloader direct map. The fields needed later
  are copied into `arch::acpi::AcpiState`; the kernel does not retain raw
  pointers.

## 2. Parsed tables

| Table | Fields | Use |
|---|---|---|
| FADT | PM1a/PM1b control, PM1 control length, SLP_TYPa/b, reset register/value, SCI IRQ, DSDT | S5 shutdown and reset |
| MADT | Local APIC base, CPU entries, IOAPIC entries, interrupt source overrides | APIC/IOAPIC discovery |
| MCFG | ECAM base, segment, bus range | PCIe configuration foundation |
| HPET | base, comparator count | APIC timer calibration source |

The HPET period is **not** in the ACPI table; it is read from the HPET MMIO
capabilities register when timer calibration is implemented. No value is faked.

## 3. Power

`arch::acpi::shutdown()` writes `SLP_TYPa << 10 | SLP_EN (0x2000)` to the FADT
PM1a control port (and PM1b when present), then falls back to the QEMU
`0x604`/`0xB004` ports. If firmware still does not enter S5, the kernel logs the
failure and halts; it never reports a successful power-off.

`arch::acpi::reset()` writes the FADT reset register (system I/O or system memory),
then pulses `0xCF9` twice and finally the keyboard-controller reset. This is a
real reset; the POWER syscall mode 1 reaches it from user space.

The ACPI SCI power button is **not** routed in 0.6.0. No fake lid, battery,
thermal or sleep state is exposed.

## 4. Verification

`tools/acpi_test.py` boots q35 and asserts the logged RSDP/FADT/MADT/MCFG/HPET
values, then runs `shutdown` in the Terminal and requires QEMU to power off.
`tools/power_test.py` covers Terminal `reboot` and the desktop confirmation
dialog, requiring the ACPI reset path and QEMU exit under `-no-reboot`.

Observed q35 values: LAPIC `0xFEE00000`, IOAPIC `0xFEC00000` GSI 0, MCFG base
`0xE0000000`, HPET `0xFED00000` with 3 comparators, FADT PM1a `0x604`,
reset `0xCF9`, SCI IRQ 9, and five interrupt source overrides.
