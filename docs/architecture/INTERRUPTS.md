# Interrupts

Aster OS 0.6.0 boots into one of two interrupt backends:

| Backend | When | EOI | Timer |
|---|---|---|---|
| APIC/IOAPIC | MADT has a LAPIC and at least one IOAPIC, MMIO maps succeed | Local APIC (`0xB0`) | PIT, routed through IOAPIC GSI 2 / vector 32 |
| 8259 PIC | APIC unavailable (fallback) | PIC | PIT, PIC IRQ 0 |

The active backend is logged (`apic: active ...` or `apic: ... keeping the 8259
path`) and exposed as `arch::apic::backend()`.

## Boot sequence

1. GDT/IDT, PIC remap, PIT programming.
2. PMM/VMM initialisation.
3. `arch::apic::init()` reads the ACPI MADT state, maps the LAPIC and IOAPIC
   windows, sets the LAPIC spurious vector and masks every IOAPIC redirection.
4. Three legacy ISA sources are routed: IRQ 0 -> GSI 2 -> vector 32, IRQ 1 ->
   GSI 1 -> vector 33, IRQ 12 -> GSI 12 -> vector 44, honouring MADT interrupt
   source override polarity/trigger bits.
5. The 8259 sources are masked only after all routes are programmed; the PIC
   path is never half-disabled.
6. Interrupts are enabled later by the scheduler.

## Handler discipline

- The timer handler increments the PIT tick, EOIs, then calls the scheduler.
- Keyboard and mouse handlers call the PS/2 driver and EOI the LAPIC.
- Unhandled vectors in 32..47 EOI the active backend; the APIC spurious vector
  is 47 and uses the same default path.
- No interrupt handler allocates, blocks or copies large buffers.
- Vectors above 47 have no generated stubs yet. MSI and a dynamic vector
  allocator are **not implemented**; see the known limitations.

## Proof

`tools/apic_test.py` asserts the LAPIC/IOAPIC log lines, all three routes, the
first real timer/keyboard/mouse interrupts and the first desktop frame. The
existing `test.py`, `ui_test.py`, `pointer_test.py`, `wm_probe.py`, `net_test.py`
and `persistence_test.py` pass on the APIC path.

## Not implemented in 0.6.0

- Local APIC timer calibration and use as the scheduler tick. The PIT remains
  the absolute tick source, now delivered through the IOAPIC.
- MSI/MSI-X capability programming.
- Interrupt-driven e1000/AHCI. Both drivers still poll and keep their existing
  counters and timeouts.
- IPIs, SMP and a dynamic vector allocator.
