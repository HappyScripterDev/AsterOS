# Aster OS 0.6.0 - Performance Notes

Only measured values are listed. QEMU TCG wall time is not representative of
hardware and no hardware benchmark was run.

## Boot and desktop

Measured from the `acpi_test`/`apic_test` serial logs on this host (q35, 1 vCPU,
512 MiB, TCG):

| Metric | Value |
|---|---|
| Kernel/network ready (DHCP bound) | ~0.63 s |
| First desktop frame | ~0.62-0.79 s |
| First frame draw | 9-13 ms |
| First frame present | 1-2 ms |
| First frame dirty area | 1,024,000 px |
| Idle frames | skipped when nothing is dirty |

## Interrupt path

- Local APIC and IOAPIC are active; the PIC is masked.
- PIT remains the 1 kHz scheduler source, now routed through IOAPIC GSI 2.
- Timer, keyboard and mouse interrupts were observed on vectors 32, 33 and 44.
- No interrupt latency microbenchmark exists; no number is claimed.

## Network and storage

- `net_test.py` and `persistence_test.py` pass unchanged on the APIC path.
- e1000 is still polled at 10 ms (100 Hz); AHCI still uses bounded command
  spin-waits. No before/after throughput claim is made because the drivers were
  not converted to interrupts.
- The baseline 0.4/0.5 throughput and AHCI timings remain the only measured
  storage/network figures and are unchanged.

## Unmeasured (absent features)

MSI, the APIC timer, USB input latency, Device Manager enumeration time and PS/2
wheel latency are not implemented and therefore have no measurements.

## Update: APIC timer and interrupt-driven I/O

- HPET period 10,000,000 fs (100 MHz); APIC timer calibrated at ~62,988-63,408
  ticks/ms depending on the run.
- First desktop frame after the switch: ~1.21 s under TCG, 88 ms draw, 14 ms
  present. Later idle frames report 20-24 ms draw in the sampled logs.
- AHCI MSI completion: persistence write/reboot/read passes with the interrupt
  flag; no command timeout was logged.
- e1000: first RX interrupt `ICR 0x83`; DHCP binds after the interrupt is armed.
  RX is drained on the interrupt flag with a 1 s watchdog.
- No interrupt-latency microbenchmark is claimed.
