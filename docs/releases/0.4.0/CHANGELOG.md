# Aster OS 0.4.0 - Changelog

Release: **0.4.0 networking**
Date: 2026-11-21
Branch: `milestone/aster-0.4.0`

## Kernel

- **New `kernel/src/net/` module** with interface state, ARP, IPv4, ICMP, UDP,
  DHCP, DNS, TCP and the user socket table.
- **Intel 82540EM (e1000) driver** (`kernel/src/drivers/e1000.rs`):
  - PCI discovery (`8086:100e`), BAR0 mapping through `vmm::map_kernel_mmio`.
  - Controller reset, link-up via `CTRL.SLU`, MAC from EEPROM.
  - 256 RX descriptors with 2 KiB buffers, 32 TX descriptors.
  - Synchronous TX completion, RX polling, first-packet diagnostics.
  - RX/TX/error/drop statistics.
- **Ethernet/ARP**: Ethernet II framing, 60 s ARP cache with 16 entries,
  request/reply, broadcast handling.
- **IPv4/ICMP**: header validation and checksum, DF packets, TTL 64,
  fragmentation explicitly dropped, echo request/reply and a blocking ping
  helper with sub-millisecond RTT reporting.
- **UDP**: client sockets, DHCP client (discover/offer/request/ack, lease
  renewal, retry policy) and DNS client (A records, retries, cache).
- **TCP client**: three-way handshake, MSS 1460, 64 KiB send queue, 128 KiB
  receive window, partial in-order acceptance, ACK processing, retransmission
  with a doubling timer, FIN close and RST handling.
- **Socket layer**: per-process socket ownership, ephemeral ports, UDP and TCP
  sockets, cleanup when a process exits.
- **Network thread**: cooperative kernel thread polling the NIC at 100 Hz and
  running DHCP/DNS/TCP timers. No NIC interrupts are used.
- **Version banner** is now `Aster Kernel 0.4.0 (networking)`.

## ABI

New syscalls (numbers 37-46):

| Number | Name | Purpose |
|---|---|---|
| 37 | `NET_STATUS` | Write a `NetStatus` structure |
| 38 | `NET_CONFIG` | DHCP renew or static IPv4 configuration |
| 39 | `NET_PING` | ICMP echo with timeout and RTT output |
| 40 | `NET_DNS` | Resolve an A record |
| 41 | `SOCKET` | Create a UDP (1) or TCP (2) socket |
| 42 | `BIND` | Bind a local port |
| 43 | `CONNECT` | Set the peer / perform the TCP handshake |
| 44 | `SEND` | Send bytes |
| 45 | `RECV` | Receive bytes with timeout |
| 46 | `SOCKET_CLOSE` | Close a socket |

New errno values: `EADDRINUSE` (-19), `ENOTCONN` (-20), `ECONNRESET` (-21),
`ECONNREFUSED` (-22).

`NetStatus` and `NetConfig` were added to `shared/aster-abi`.

## Bootloader

- Loads `NETPROBE.ELF` as the sixth module when present.

## Desktop

- **Menu bar**: real Ethernet icon and tooltip (`Ethernet 10.0.2.15`,
  `Link down`, `No wired adapter`); click opens the Network app.
- **Control Centre**: live Ethernet tile showing the current IPv4 address or
  link state instead of a static Wi-Fi row.
- **Network app** (new, tenth dock app): adapter/link/method/address/mask/
  gateway/DNS/MAC/traffic/lease rows, DHCP renew, gateway ping, and an input
  field for ad-hoc ping or DNS queries with results.
- **Settings > Network**: real adapter and lease information, traffic counters
  and a Renew DHCP button.
- **Monitor**: a Network card (address or link state) and an `eth0` detail line
  with MAC, RX/TX packet counts and errors.
- **Terminal**: new `net`, `ping <ipv4>` and `dns <name>` diagnostics commands.
- About/Terminal versions updated to 0.4.0.

## Tests and tools

- `tools/net_test.py`: boots with an explicit e1000 and QEMU user-mode
  networking, runs host TCP/UDP/DNS servers, spawns `netprobe`, and verifies
  DHCP, ICMP, TCP echo, 64 KiB TCP bulk echo, UDP echo and DNS.
- `tools/network_ui_probe.py`: captures the Network app after a live ping and
  the Settings network page.
- `tools/app_probe.py`: includes the Network app.
- `tools/test.py`: boot smoke now asserts the e1000 and a DHCP lease.
- `tools/run.py`: `net_args()` and an explicit e1000 on every QEMU launch.

## Documentation

- `docs/releases/0.4.0/` with baseline, roadmap, network architecture, test
  report, performance, limitations and IPv6/TLS architecture notes.
- `docs/PROJECT_STATUS.md` and `README.md` updated.
