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
