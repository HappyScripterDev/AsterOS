# Aster OS - Project Status

Version: **0.4.0 networking**
Date: 2026-11-21
Kernel: Aster 0.4.0 (higher half, x86_64)
ABI: Aster ABI 1 (syscalls 1-46)
Desktop: Aster Desktop (ring 3, light macOS-inspired shell, software compositor)

## Build and test status

| Check | Result |
|---|---|
| `python tools/build.py` | PASS (bootloader, kernel, desktop, 512 MiB image verified) |
| Host image tests (`tests/`) | PASS |
| Boot smoke (`tools/test.py`) | PASS (boot, e1000 detection, DHCP lease, desktop, no faults) |
| Network acceptance (`tools/net_test.py`) | PASS (DHCP, ICMP, TCP echo, 64 KiB TCP bulk echo, UDP echo, DNS) |
| Network UI (`tools/network_ui_probe.py`) | PASS (live Network app ping, Settings network page) |
| UI harness (`tools/ui_test.py`) | PASS |
| App probe (`tools/app_probe.py`) | PASS (10 applications captured, including Network) |
| Pointer/dock test (`tools/pointer_test.py`) | PASS |
| Window drag + virtual desktops (`tools/wm_probe.py`) | PASS |
| Persistence (`tools/persistence_test.py`) | PASS (write, reboot, read; host FAT32 verifier) |
| Settings reliability (`tools/settings_test.py`) | PASS |
| Service lifecycle (`tools/service_test.py`) | PASS |
| Files operations (`tools/files_probe.py`) | PASS |
| Filesystem probe (`tools/fs_probe.py`) | PASS |
| Storage probe (`tools/storage_probe.py`) | PASS |
| Isolation (`tools/isolation_test.py`) | PASS |
| Status/menu probes (`status_probe.py`, `shell_probe.py`, `about_probe.py`, `final_ui_probe.py`) | PASS |
| Benchmark (`tools/bench.py`) | Recorded (see `docs/releases/0.4.0/PERFORMANCE.md`) |
| CI | Workflow present; not executed on GitHub infrastructure |

## Milestone tracker

| Phase | State | Notes |
|---|---|---|
| 0 Research/architecture | COMPLETED | Research, ADRs, design system, threat model |
| 1 Build infrastructure | COMPLETED | Reproducible image, QEMU runner, firmware fetch, tests |
| 2 First boot | COMPLETED | UEFI -> higher-half kernel -> serial |
| 3 Core kernel | PARTIAL | Memory, interrupts, scheduler, syscalls, spawn/wait, IPC, persistence; APIC/SMP and demand paging missing |
| 4 Drivers/user space | PARTIAL | Serial, GOP, PS/2, PCI, AHCI, e1000; ACPI/APIC, USB, audio missing |
| 5 Graphics/desktop | PARTIAL | Compositor, WM, shell, settings, network UI; damage tracking, HiDPI, accessibility, GPU missing |
| 6 Applications | PARTIAL | 10 real apps; Store/Installer/Update/Browser deliberately absent |
| 7 Networking | PARTIAL | Real e1000 + IPv4 client stack, DHCP/DNS/TCP/UDP, sockets and UI; IPv6/TLS not implemented, polling only |
| 8 Advanced | NOT STARTED | USB, audio, NVMe, power, SDK |
| 9 Release | NOT STARTED | Optimization, hardware validation, installer media |

## Component status

| Component | State | Evidence / limitation |
|---|---|---|
| Aster Boot (UEFI) | COMPLETE for milestone | Loads kernel + six modules, builds page tables, ExitBootServices |
| GPT/FAT32 image | COMPLETE | Deterministic builder + verifier; 8.3 names only |
| GDT/TSS/IDT | COMPLETE | 256 vectors, IST double fault, DPL3 syscall gate |
| PMM | COMPLETE (32 GiB cap) | Bitmap, zeroing, reservations, accounting |
| VMM | COMPLETE for milestone | 4-level, U/S on all levels, per-process PML4, NX/W^X, MMIO mapping |
| Kernel heap | COMPLETE | first-fit, coalescing, GlobalAlloc, self-test |
| Scheduler | PARTIAL | Preemptive user threads, zombies, WAIT/IPC wakeups, cooperative kernel threads, no SMP |
| Syscalls | PARTIAL | 46 numbers assigned, 45 implemented; SET_THEME returns ENOSYS |
| Filesystem | PARTIAL | VFS + mount table, ramfs root and FAT32 home; 8.3 names, no journal/permissions |
| PS/2 input | COMPLETE for milestone | IRQ keyboard/mouse, ASCII + special keys |
| Framebuffer | COMPLETE | GOP 1280x800, mapped into the desktop |
| Desktop shell | PARTIAL | Light menu bar, Spotlight, dock, Control Centre, notifications, live network status |
| Window manager | PARTIAL | Traffic lights, drag/resize/maximize/snap/minimize, animations; no alt-tab overlay |
| Applications | PARTIAL | 10 functional apps including Network diagnostics |
| Security | PARTIAL | Ring 3, W^X, NX, pointer validation; no ASLR/SMEP/SMAP/sandbox/users, no network privileges |
| Settings | IMPLEMENTED | Versioned, validated, backup/restore, configd service; network page is read-only except DHCP renew |
| Services | PARTIAL | configd + supervisor; no generic manager or KILL |
| Logging | PARTIAL | Levels + timestamps + subsystem prefixes; LOG_READ + Monitor view; volatile ring |
| Networking | PARTIAL | e1000, Ethernet/ARP/IPv4/ICMP/UDP/DHCP/DNS/TCP client, sockets 37-46, live UI; polling, IPv4 only, no inbound TCP |
| Audio | NOT STARTED | No HDA/AC'97 driver; UI states this |
| Storage | PARTIAL | Block registry, mount table, GPT/FAT32, STORAGE_DETAILS; single-sector DMA |
| Installer/updates | NOT STARTED | Explicitly absent from the UI |
| SDK | NOT STARTED | ABI documented; no package template yet |

## Known issues

1. System files remain in ramfs. `/home/user` is persistent only when the data disk is attached; the volume uses 8.3 names and has no journal.
2. Built-in apps share the desktop address space; an app crash takes down the shell. Spawned processes are isolated, but apps have not moved yet.
3. Desktop startup prepares wallpapers/icons and composes the first frame in roughly 3.5-4.9 s under QEMU TCG.
4. Software composition runs at roughly 22-50 ms per frame at 1280x800 under TCG depending on open windows; no damage tracking.
5. User binaries are linked at 4 MiB because the toolchain uses the small code model.
6. No keyboard typematic repeat or mouse wheel events in the guest yet.
7. The ESP uses 8.3 file names; the loader does not parse long file names.
8. No guard pages around kernel/user stacks; overflow would corrupt adjacent memory.
9. No window resize automated test yet (drag and desktop switching are covered).
10. Toast notifications overlap windows until they expire.
11. FAT32 has no journal or permissions; a power loss can truncate a file.
12. IPC messages are 8 x 256 bytes, receive-timeout only; no shared memory or handle passing.
13. One managed service (configd) and no KILL.
14. Files copy is depth-limited with no progress UI; search is per-directory.
15. The network stack is IPv4-only, polls the NIC, has no inbound TCP listener, no IPv6/TLS, and does not persist static configuration.
16. Any process can open sockets or call `NET_CONFIG`; there is no capability model or firewall.
17. UDP checksums are sent as zero and not validated.
18. The Calendar and top bar depend on QEMU's `-rtc base=localtime`.

## Next tasks (ordered)

| ID | Task | State | Acceptance |
|---|---|---|---|
| N1 | APIC/IOAPIC + NIC interrupts | NEXT | e1000 RX/TX completes without the 10 ms polling floor |
| N2 | Listening TCP + in-guest server | NEXT | Guest serves a TCP connection; test connects from host |
| N3 | Persistent network configuration | NEXT | DHCP/static mode and DNS survive reboot through configd |
| N4 | IPv6 dual stack | NOT STARTED | SLAAC, ND, ICMPv6, AAAA, TCP over IPv6 |
| N5 | TLS 1.3 in user space | BLOCKED on CSPRNG | `getrandom`, reviewed primitives, certificate validation |
| K3 | Move apps to separate processes | NEXT | Closing/crashing an app leaves the desktop alive |
| K4 | Guard pages for stacks | NOT STARTED | Overflow triggers a clean fault, not corruption |
| K6 | Per-process memory quotas | NOT STARTED | A child cannot exhaust physical memory |
| K5 | SMEP/SMAP/UMIP | NOT STARTED | Kernel exploit tests fail; feature logged at boot |
| D1 | ACPI/MADT discovery | PARTIAL | PCI and AHCI work; ACPI/MADT missing |
| U1 | Damage tracking | NOT STARTED | Idle composition under 5% CPU in QEMU |
| A1 | Archive app + format | NOT STARTED | Create/extract with integrity check |

## Metrics snapshot

- Source: 42,282 tracked lines (Rust 27,430 in 67 files; Markdown 10,241;
  Python 4,193). `python tools/loc.py` reports the current count.
- Kernel image: ~3.03 MiB ELF (debug info included).
- Desktop image: ~4.05 MiB ELF.
- Boot: 3.47 s to first frame, kernel ready at 1.81 s (`tools/bench.py` run on
  2026-11-21; TCG variance is several hundred ms).
- Idle after desktop: 442 MiB free of 510 MiB; idle frame 22.6 ms.
- Storage: 64 KiB FAT32 write ~19 ms, read ~17 ms (single-sector AHCI).
- Network: DHCP bound at 0.70 s; 64 KiB TCP echo round trip 11-12 ms; DNS query
  10 ms; ping RTT below 1 ms resolution.

## Release gate for 1.0

Aster OS will not be called a usable desktop OS until, at minimum:

- user processes can be spawned, isolated, and sandboxed;
- files persist across reboots with crash consistency;
- networking, IPv6 and TLS work and are verified on real hardware;
- the desktop meets its accessibility and performance targets on real hardware;
- security claims are backed by tests and an external review;
- the installer produces a bootable system from release media.
