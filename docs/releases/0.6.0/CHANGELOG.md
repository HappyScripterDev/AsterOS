# Aster OS 0.6.0 - Changelog

## Phase 1 - ACPI core and power

- Added `arch/acpi.rs`: validated RSDP (including EBDA fallback), XSDT/RSDT
  walk, FADT, MADT, MCFG and HPET parsing with length and checksum checks.
- q35 discovery is logged with LAPIC, IOAPIC, interrupt source overrides, MCFG
  base and HPET base/comparators. No AML is executed.
- Added ACPI S5 shutdown and FADT reset with legacy fallbacks; both halt and log
  instead of pretending to power off if firmware does not act.
- Added syscall 47 `POWER` (0 shutdown, 1 restart). Legacy syscall 28 now aliases
  shutdown.
- Added Terminal `shutdown`/`reboot` and a desktop App-menu Restart/Shut Down
  confirmation dialog with Cancel, Enter and Escape handling.
- Added `tools/acpi_test.py` (tables plus S5) and `tools/power_test.py`
  (Terminal/desktop restart). Both pass on q35.
- Added `docs/architecture/ACPI.md` and updated the syscall/feature/status docs.

## Phase 2 - Local APIC and IOAPIC (partial)

- Added `arch/apic.rs`: map the LAPIC/IOAPIC, enable the LAPIC through the APIC
  base MSR, set the spurious vector, mask all IOAPIC entries and route ISA IRQs
  0/1/12 with MADT override polarity/trigger.
- Mask the 8259 only after all IOAPIC routes are programmed; the PIC path remains
  a fallback when the MADT or MMIO setup fails.
- Interrupt EOI now goes to the Local APIC when active; PIC EOI is used only in
  fallback mode. Timer/keyboard/mouse first-IRQ proof logs are emitted.
- PIT remains the 1 kHz scheduler source, delivered through IOAPIC GSI 2.
- Added `tools/apic_test.py`; existing boot/UI/pointer/WM/network/persistence
  tests pass on the APIC path.
- Not completed in this phase: MSI/MSI-X, APIC timer, interrupt-driven e1000 and
  AHCI. See `KNOWN_LIMITATIONS.md`.
