# Aster OS Feature Matrix

Status values: **Implemented**, **Partial**, **Not implemented**. "Tested"
means the feature is exercised by an automated test in this repository or was
verified during the recorded milestone session. Limitations are explicit.

| Feature | Status | Tested | Limitations / Evidence |
|---|---|---|---|
| UEFI boot (custom loader) | Implemented | Yes | `tools/test.py` sees `BOOT-HANDOFF-OK`; OVMF only, no Secure Boot policy |
| GPT + FAT32 ESP | Implemented | Yes | Host tests verify CRC, cluster chains, contents; 8.3 names only |
| Kernel ELF loading | Implemented | Yes | PT_LOAD validation, page-aligned segments, BSS zeroing |
| Higher-half kernel | Implemented | Yes | Image at `0xFFFFFFFF80000000`, direct map at `0xFFFF800000000000` |
| GDT/TSS/IDT | Implemented | Yes | Ring 3 transitions, TSS RSP0, IST double fault |
| Exceptions | Implemented | Yes | Registers + CR2, user faults kill the process, fatal kernel faults halt |
| PIC + PIT | Implemented | Yes | 1 kHz PIT; IRQ 0 routed through IOAPIC GSI 2 when APIC is active; PIC fallback |
| PMM | Implemented | Yes | Bitmap, 32 GiB max, reservations, zeroing |
| VMM / paging | Implemented | Yes | Per-process PML4, NX, W^X, U/S at every level |
| Kernel heap | Implemented | Yes | First-fit, 48-256 MiB virtual limit, self-test |
| Scheduler | Partial | Yes | Preemptive user threads; kernel threads cooperative; single CPU |
| Context switching | Implemented | Yes | Callee-saved + FXSAVE, CR3 switch |
| System calls | Partial | Yes | 48 numbers; POWER 47, DEVICES 48 (PCI inventory), legacy SHUTDOWN 28; SET_THEME returns ENOSYS |
| Ring-3 processes | Implemented | Yes | Desktop PID 1 plus spawned children; private PML4, 64 KiB stack, W^X ELF |
| IPC | Implemented | Yes | Bidirectional channels, handles, blocking send/recv with receive timeout, close/hangup; 8x256 B queues |
| Process creation API | Implemented | Yes | `SPAWN`/`WAIT`; isolation test spawns a normal and a faulting child |
| Serial console | Implemented | Yes | COM1, 115200 8N1, ring buffer |
| Framebuffer | Implemented | Yes | GOP 1280x800, user mapping |
| PS/2 keyboard | Implemented | Yes | ASCII + special keys, modifiers; no typematic repeat in guest |
| PS/2 mouse | Implemented | Yes | Absolute pointer, buttons, IntelliMouse 4-byte wheel; Files scrolls on the wheel |
| PCI configuration | Partial | Yes | Mechanism #1; AHCI found by class, e1000 by vendor/device; full enumeration/ACPI planned |
| ACPI | Partial | Yes | RSDP/XSDT/FADT/MADT/MCFG/HPET validated; S5 shutdown and reset; no AML interpreter or SCI power button |
| Power management | Partial | Yes | ACPI S5 and FADT reset via POWER 47; Terminal and confirmed desktop menu; no sleep/battery/thermal |
| Device inventory | Implemented | Yes | `DEVICES` 48, Terminal `devices`, Settings > Devices; real PCI functions only |
| APIC / SMP | Partial | Yes | LAPIC, IOAPIC, APIC timer (HPET-calibrated), dynamic vectors, MSI for AHCI and PCI INTx for e1000; no IPIs or SMP |
| VirtIO block/network | Not implemented | - | AHCI is used for the data disk; no VirtIO devices |
| AHCI (q35) | Implemented | Yes | MSI completion interrupts, bounded polling fallback, IDENTIFY, single-sector DMA read/write, FLUSH CACHE EXT |
| Intel 82540EM (e1000) | Implemented | Yes | MMIO BAR0, EEPROM MAC, 256 RX / 32 TX descriptors, RX/TX IOAPIC INTx interrupt, IFCS; QEMU 82540EM has no MSI cap |
| Ethernet / ARP / IPv4 / ICMP | Implemented | Yes | ARP cache, header checksum, echo; no fragmentation/reassembly |
| UDP / DHCP / DNS | Implemented | Yes | DHCP lease, DNS A records; UDP checksum not validated, no DHCPv6 |
| TCP client | Implemented | Yes | Handshake, sliding window, retransmit, FIN/RST; no listen/accept, no SACK |
| Network sockets | Implemented | Yes | Syscalls 37-46; blocking with timeouts; no select/poll/options |
| VFS / mounts | Implemented | Yes | Block registry + mount table; ramfs `/` and FAT32 `/home/user`; longest-prefix routing |
| Native AFS | Not implemented | - | Design not started |
| Persistence | Implemented | Yes | FAT32 home survives reboot; versioned settings with backup; write/reboot/read + host verifier |
| File permissions | Not implemented | - | The persistent volume is world-readable/writable; 0644/0755 are informational |
| Menu bar | Implemented | Yes | Brand tile, app name, working menus with open/close fade, live Ethernet status and clock |
| Dirty-region compositor | Implemented | Yes | Bounded merged regions, partial wallpaper restore/present, idle frame skipping, full-redraw fallback |
| Frame timing / telemetry | Implemented | Yes | Delta/interval timing, 30/60/120/Free pacing, real FPS/draw/present/dirty counters in Monitor |
| Graphics quality modes | Implemented | Yes | Quality/Balanced/Performance/Adaptive change shadow blur and dirty-region limits |
| Motion engine | Implemented | Yes | `aster-motion` easing/tweens/springs/tracks/sequences with host unit tests |
| Window animations | Implemented | Yes | Open/close/minimize-to-dock/restore/maximize/snap/focus with real opacity; state machine |
| Window Overview | Implemented | Yes | F3 overview with captured real previews, labels, minimized cards, keyboard/mouse |
| Quick switcher | Implemented | Yes | Held-Alt overlay with icons/titles/workspaces; release-to-switch |
| Dock 2.0 | Implemented | Yes | Bottom/left/right, size, magnification, auto-hide, running/minimized indicators |
| Workspace transitions | Implemented | Yes | Horizontal slide, indicators, keyboard/menu switch |
| Window tabs | Partial | Yes | Text editor tabs create/select/close/shortcuts; no drag reorder/tear-out |
| Effects | Partial | Yes | Real shadows/transparency/alpha; blur is cached wallpaper blur, not live backdrop |
| Spotlight launcher | Implemented | Yes | Ctrl+Space, translucent search panel, keyboard navigation, dynamic results |
| App icon set | Implemented | Yes | Ten original squircle icons; nine generated by tools/icongen.py, Network drawn in ui.rs |
| Runtime display scaling | Not implemented | - | `Display`/`DisplaySet` and a persisted ui_scale foundation exist; UI renders at 1.0 |
| GPU acceleration | Not implemented | - | No GPU backend; Monitor reports the software renderer only |
| Appearance | Implemented | Yes | Light-only, accent colours, four procedural wallpapers, reduce motion, brightness |
| Terminal | Implemented | Yes | Real shell over ramfs; help/ls/cat/write/mkdir/rm/ps/free/date/appearance/accent/open/net/ping/dns |
| Files | Implemented | Yes | Sidebar (Home/Documents/Downloads/Desktop/Notes/System/Logs/Root), breadcrumbs, forward/back, new folder, rename, delete confirm, copy/move/paste, info, sort, search |
| Settings | Implemented | Yes | Appearance/Display/Sound/Network/Bluetooth/Users/Security/Storage/Accessibility/Graphics & Motion/About; live network/graphics state; v1 validated settings through configd |
| Calculator | Implemented | Yes | Arithmetic parser, keyboard + mouse |
| Monitor | Implemented | Yes | SystemInfo cards, network and graphics telemetry (FPS/frame/draw/present/dirty), storage usage/free, process list, kernel log view |
| Text editor | Implemented | Yes | Create/open/save, keyboard editing, no selection/undo |
| Notes | Implemented | Yes | Notes directory, create/delete/save |
| Calendar | Implemented | Yes | CMOS RTC, month navigation |
| Wallpapers | Implemented | Yes | Four light procedural wallpapers (Sonoma/Sequoia/Ventura/Monterey), previews and apply; no image file decoding |
| Store / Installer / Update | Not implemented | - | Deliberately absent from UI |
| Browser | Not implemented | - | Networking exists, but no browser engine integration |
| Networking | Partial | Yes | e1000 + IPv4 client stack, DHCP/DNS/TCP/UDP sockets, live UI; polling, IPv4 only, no inbound TCP, IPv6 or TLS |
| Users/login | Not implemented | - | Single implicit `user`, no password |
| Sandboxing | Partial | Yes | Spawned processes/children are isolated; configd is a separate process; built-in apps still share the desktop |
| Services | Partial | Yes | configd settings service, supervisor restart, timeout and direct-I/O fallback; no generic manager/KILL |
| Logging | Partial | Yes | Levels/timestamps/subsystem prefixes, LOG_READ and Monitor log view; volatile ring |
| ASLR/KASLR | Not implemented | - | Fixed addresses |
| SMEP/SMAP/UMIP | Not implemented | - | CPU features not enabled |
| Signed boot/packages/updates | Not implemented | - | No crypto stack |
| Audit logging | Partial | - | Kernel ring log; not exposed as a service |
| Package manager | Not implemented | - | - |
| SDK / app template | Not implemented | - | ABI documented; no SDK |
| Installer media | Not implemented | - | Disk image is hand-built |
| Automated tests | Partial | Yes | Host, motion unit, boot, UI, storage, isolation, network and graphics/pixel tests; gaps listed in TESTING.md |
| CI | Partial | No | Workflow exists, not yet run on GitHub |
| Documentation | Implemented | - | Architecture, ADRs, research, design, security, benchmarks |

## Honest notes

- "Implemented" means the feature performs its advertised function; it does not
  mean it is complete relative to a production OS.
- No feature in this matrix is represented by a decorative control. The
  Settings app explicitly reports missing audio, network, and Bluetooth
  hardware/drivers; Store/Installer/Update have no buttons anywhere.
- The desktop is intentionally light-only. There is no dark mode or theme
  switch; the Control Centre and Settings state this explicitly.
- The Edit menu contains disabled standard items (Undo/Cut/Copy/Paste) because
  text selection and clipboard services are not implemented. Disabled items
  are rendered inert; they are not advertised as working.
- The Wallpapers app is named as such rather than "Photos" because image file
  decoding is not implemented.
- Persistent storage does not mean crash-consistent storage: there is no
  journal, and the volume uses 8.3 names. These limits are shown in Settings. It previews generated wallpapers that it can
  actually apply.
