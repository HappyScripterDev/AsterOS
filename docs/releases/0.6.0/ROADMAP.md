# Aster OS 0.6.0 - Platform & Devices

Status: planned. Baseline: 0.5.0 graphics and motion (2026-11-21).

## Product direction

- 0.6.0 is the platform foundation: ACPI, APIC/IOAPIC, MSI, interrupt-driven
  devices, USB HID, power management, a device manager and mouse-wheel input.
- Audio, Bluetooth and SMP are 0.6.1. HiDPI, VirtIO-GPU, Stage Manager and
  app-internal damage are 0.6.2. They are not reopened in 0.6.0.
- The UI remains the light glass desktop from 0.5. No dark mode and no visual
  rewrite in this release.
- No fake battery, thermal, sleep, USB or GPU state. If the hardware or ACPI
  does not expose a capability, the UI says so.

## Goals

1. Parse the ACPI fixed tables and use them for CPU, interrupt and power
   discovery.
2. Replace the PIC-only interrupt path with Local APIC, IOAPIC and MSI where
   available; keep a PIC fallback.
3. Move e1000 and AHCI from polling to interrupts without regressing network or
   storage behaviour.
4. Bring up USB xHCI with HID keyboard/mouse and hotplug.
5. Add ACPI shutdown/restart, a power menu and a real device manager.
6. Add PS/2 mouse-wheel events and smooth scrolling.
7. Keep the existing desktop, networking, storage, services and tests intact.

## Non-goals

- HDA audio, Bluetooth and SMP (0.6.1).
- HiDPI, multi-monitor, VirtIO-GPU, Stage Manager, app-internal damage and the
  resource cache (0.6.2).
- A full AML interpreter or arbitrary ACPI device drivers.
- USB isochronous transfers, USB audio, or a complete hub tree beyond one
  external hub.
- Vendor GPU drivers, shaders or GPU rasterisation.
- S3 sleep if QEMU/ACPI does not expose it. Document the gap instead of faking
  a sleep button.

## Starting point

- `milestone/aster-0.5.0-gpu` is committed and clean; the annotated `v0.5.0`
  tag must not move.
- `BootInfo.rsdp_address` is already filled by the UEFI loader.
- The kernel uses PIC/PIT, polling e1000 and polling AHCI.
- PS/2 keyboard and mouse work; the mouse has no wheel events.
- There is no ACPI parser, APIC, IOAPIC, MSI, USB, audio or power management.
- Some 0.5 docs still describe the pre-extension compositor; correct them in
  Phase 0.

## Phase 0 - Audit and baseline

- Create a new branch from the 0.5 tip; do not commit on the 0.5 branch.
- Build and run the full existing suite.
- Correct stale 0.5 documentation: live glass, per-window surfaces, window
  damage, SSE2 row blits, overview timing and the actual test list.
- Record baseline: boot time, PIC interrupt counts, e1000 polling interval and
  throughput, AHCI polling I/O, missing USB/audio/power, device inventory.
- Write `BASELINE.md`.

Gate: build green, existing tests green, baseline documented, docs match code.

## Phase 1 - ACPI core

- Validate the RSDP from `BootInfo`; prefer XSDT, fall back to RSDT; validate
  checksums and table lengths.
- Parse FADT (PM1a/PM1b control and status, reset register, SCI), MADT (Local
  APIC base, IOAPIC entries, interrupt source overrides), MCFG (PCIe ECAM) and
  HPET (base, comparator, period).
- Map tables through the direct map; copy or reserve them before the memory
  map is reused. Never trust table lengths.
- Do not implement an AML interpreter in 0.6.0. Log DSDT/SSDT presence only when
  a fixed table is not enough.
- Add ACPI S5 shutdown and reset through the FADT PM1 control registers. Expose
  one `POWER` syscall (next free number, 47) with shutdown/restart modes.
- Add Terminal `shutdown`/`reboot` and a desktop power menu with confirmation.
- If the ACPI SCI power button is feasible, route it and shut down cleanly;
  otherwise document it as missing.
- Gate: QEMU q35 reports the expected CPU count, LAPIC, IOAPIC, HPET and MCFG;
  shutdown and restart work from Terminal and the desktop; no triple fault and
  no hang.

## Phase 2 - APIC, IOAPIC, MSI and interrupt-driven drivers

- Local APIC: enable through MSR, configure the spurious vector, EOI, calibrate
  the APIC timer with HPET/PM timer/TSC, and prepare IPI delivery for 0.6.1.
- IOAPIC: map MMIO, read version and redirection entries, program ISA IRQs for
  keyboard/mouse, honour interrupt source overrides (polarity/trigger).
- Replace the fixed IDT IRQ stubs with a vector allocator and a handler
  registry. Keep vectors 32-47 as a compatibility range, but do not assume
  every device uses them.
- EOI policy: Local APIC EOI for all external interrupts when APIC is active;
  no PIC EOI in that mode. Keep the PIC path behind a fallback flag.
- MSI/MSI-X for e1000 and AHCI through PCI capability parsing. If MSI fails,
  fall back to IOAPIC INTx and then to polling; log which path is active.
- e1000: enable RX/TX interrupts, wake the network thread, remove the 100 Hz
  polling floor, keep a 1 Hz watchdog and counters.
- AHCI: enable completion interrupts, remove polling, keep timeouts and error
  recovery.
- Add IRQ counters and the active interrupt backend to Monitor and logs.
- Gate: DHCP/ICMP/TCP/UDP/DNS tests pass on the interrupt path; AHCI
  persistence tests pass; no interrupt storm; keyboard and mouse still work;
  the timer still preempts.

## Phase 3 - USB xHCI and HID

- Discover the xHCI controller by PCI class (0x0C/0x03/0x30), map BAR0 and
  reset the controller.
- Set up the command ring, event ring, primary interrupter and scratchpad
  buffers; enable MSI or IOAPIC INTx.
- Implement device slots, address-device, descriptor reads, endpoint
  configuration, control/bulk/interrupt transfers and port status changes.
- HID boot keyboard and mouse: parse the report descriptor only as far as the
  boot protocol requires, translate reports into the existing input event
  queue, and support hotplug.
- Keep PS/2 as a fallback and avoid duplicate events when both paths exist.
- Mass storage is optional in 0.6.0. If it is not finished, document it and do
  not add a fake device.
- QEMU: add `python tools/run.py --usb`, attach `qemu-xhci`, `usb-kbd` and
  `usb-mouse`; tests must exercise hotplug.
- Gate: USB keyboard and mouse work in QEMU, hotplug works, PS/2 still works,
  and the device manager lists the controller and devices.

## Phase 4 - Power, device manager and input polish

- Power menu: Shutdown and Restart with a confirmation; Settings > Power shows
  the ACPI backend and the supported actions. No sleep button unless S3 works.
- Device manager: enumerate PCI, USB and ACPI-described devices; show vendor,
  device, class, driver, IRQ/MSI, and status. Add Settings > Devices and a
  Monitor card.
- Hardware inventory: CPU count, ACPI tables, IOAPIC, HPET, MCFG, USB
  controllers and devices. No fake sensors.
- PS/2 mouse wheel: detect IntelliMouse, parse 4-byte packets, add wheel events
  and smooth scrolling in Files, Text, Settings and other scrollable apps.
- If keyboard typematic repeat is still missing, implement it; otherwise note
  that it was already present.
- Gate: shutdown/restart from UI and Terminal; device list matches QEMU;
  wheel scrolling works; no fake battery/thermal rows.

## Phase 5 - Tests, documentation and release evidence

- New tests:
  - `tools/acpi_test.py`: RSDP/XSDT/FADT/MADT/MCFG/HPET values and shutdown.
  - `tools/apic_test.py`: APIC timer, IOAPIC routing, MSI path and IRQ counters.
  - `tools/usb_test.py`: xHCI init, HID keyboard/mouse, hotplug.
  - `tools/power_test.py`: shutdown/restart from Terminal and the UI.
  - `tools/device_probe.py`: Settings > Devices and Monitor inventory.
  - `tools/input_test.py`: wheel events and smooth scrolling.
- Update existing tests: `test.py`, `net_test.py`, `network_ui_probe.py`,
  `persistence_test.py`, `storage_probe.py`, `ui_test.py`, `pointer_test.py`,
  `wm_probe.py`, `settings_test.py`, `app_probe.py`.
- Documentation: `docs/architecture/ACPI.md`, `INTERRUPTS.md`, `USB.md`,
  `POWER.md`, `DEVICES.md`, `SYSCALLS.md` (POWER 47, device info syscall),
  `docs/FEATURES.md`, `docs/PROJECT_STATUS.md`, `docs/ROADMAP.md`, `README.md`,
  `CHANGELOG.md` and `docs/releases/0.6.0/`.
- Benchmarks: interrupt latency, network latency/throughput before and after,
  AHCI I/O, boot time and USB input latency. Record measured values only.
- Release files: `BASELINE.md`, `TEST_REPORT.md`, `PERFORMANCE.md`,
  `KNOWN_LIMITATIONS.md` and `CHANGELOG.md`.

Gate: every test green, no regressions, documentation accurate, no unsupported
claims.

## Acceptance

| Area | Criterion |
|---|---|
| Build | `python tools/build.py` passes and the image verifies |
| Boot | `python tools/test.py` passes with the new interrupt path |
| ACPI | QEMU tables parsed; CPU/IOAPIC/HPET/MCFG values logged correctly |
| Interrupts | APIC timer, IOAPIC ISA routing and MSI for e1000/AHCI work |
| Network | DHCP, ICMP, TCP, UDP and DNS pass on the interrupt path |
| Storage | AHCI read/write and persistence pass on the interrupt path |
| USB | xHCI HID keyboard and mouse work; hotplug works; PS/2 fallback |
| Power | Shutdown and restart work from Terminal and the desktop |
| Devices | Settings/Monitor show the real PCI/USB/ACPI inventory |
| Input | PS/2 wheel and smooth scrolling work |
| Honesty | No fake audio, Bluetooth, battery, thermal, sleep or GPU state |
| Tests | Existing and new tests green |
| Docs | Architecture, feature, status, roadmap, README and release docs updated |

## Risks

| Risk | Mitigation |
|---|---|
| ACPI table bugs or bad lengths | Checksums, bounds checks, QEMU q35 fixtures, log every table |
| AML dependency for PCI routing | Use MSI/MSI-X first; fall back to IOAPIC swizzle and polling; do not build an AML interpreter |
| Interrupt storms or lost interrupts | Short handlers, EOI discipline, counters, watchdog, both APIC and PIC paths |
| e1000/AHCI regression | Keep polling fallback, run the existing network and persistence suites |
| USB is larger than expected | HID first; mass storage optional; document what is missing |
| Power actions hang QEMU | S5/reset only; serial markers; test with `-no-reboot` and timeouts |
| Scope creep | 0.6.1 and 0.6.2 carry audio/SMP and graphics; do not mix them in |

## File map

- `kernel/src/arch/acpi.rs` - RSDP/XSDT/FADT/MADT/MCFG/HPET parser and power.
- `kernel/src/arch/apic.rs` - Local APIC, timer, EOI and IPI groundwork.
- `kernel/src/arch/ioapic.rs` - IOAPIC routing and interrupt source overrides.
- `kernel/src/arch/idt.rs` - vector allocator and handler registry.
- `kernel/src/drivers/pci.rs` - MSI/MSI-X capability parsing.
- `kernel/src/drivers/e1000.rs` - interrupt-driven RX/TX.
- `kernel/src/drivers/ahci.rs` - interrupt-driven completion.
- `kernel/src/drivers/xhci.rs` - USB xHCI controller and transfers.
- `kernel/src/drivers/usb_hid.rs` - HID boot keyboard/mouse.
- `kernel/src/drivers/ps2.rs` - IntelliMouse wheel.
- `kernel/src/syscall.rs`, `shared/aster-abi/src/lib.rs` - POWER and device info.
- `user/asterdesk/src/desktop.rs`, `apps.rs`, `settings.rs` - power menu, device
  manager, Monitor inventory and wheel scrolling.
- `tools/` - new ACPI, APIC, USB, power, device and input tests.
- `docs/` - architecture, feature, status, roadmap and release documentation.
