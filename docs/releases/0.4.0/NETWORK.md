# Aster OS 0.4.0 - Network Architecture

## 1. Scope

Aster OS 0.4.0 implements one wired Ethernet controller and a real IPv4 client
stack:

```
user process
   |  SOCKET / BIND / CONNECT / SEND / RECV / SOCKET_CLOSE
   |  NET_STATUS / NET_CONFIG / NET_PING / NET_DNS
   v
kernel socket table  ->  UDP  ->  DHCP client
   |                     |  ->  DNS client
   |                     v
   |                    IPv4  ->  ICMP
   v                     |
   TCP  -----------------+
   |
   v
Ethernet II  ->  ARP
   |
   v
Intel 82540EM (e1000) driver
   |
   v
QEMU user-mode network (SLIRP) or a real Ethernet segment
```

There is no user-space network daemon in this milestone. The "network service"
is the kernel `net` thread plus the socket table; the desktop observes it
through syscalls. `configd` still owns desktop preferences, not network state.
A user-space service split is a 0.5.0 candidate.

## 2. Device layer

| Property | Value |
|---|---|
| Controller | Intel 82540EM, PCI `8086:100e` |
| Reference location | q35 bus 0, device 1, function 0 |
| Register access | MMIO BAR0, 128 KiB, mapped with `vmm::map_kernel_mmio` |
| MAC source | EEPROM words 0-2 through `EERD` |
| Link | `CTRL.SLU`, `STATUS.LU` |
| RX ring | 256 legacy descriptors, 2 KiB buffers |
| TX ring | 32 legacy descriptors, 2 KiB buffers |
| Completion | Polling; TX waits for the descriptor DD bit |
| Interrupts | Masked (`IMC = 0xffffffff`); no MSI/IOAPIC routing exists |

The driver is in `kernel/src/drivers/e1000.rs`. It exposes `present`, `mac`,
`link_up`, `send(frame)` and `receive()`, plus the counters aggregated into
`NetStats`. Descriptor memory and packet buffers come from the physical frame
allocator and are accessed through the direct map.

All TX is synchronous: the caller waits for DD before returning. This limits
burst size but makes buffer ownership simple and was the correct choice without
interrupts. RX is drained in a loop by the network thread.

## 3. Network core

`kernel/src/net/mod.rs` owns:

- interface state (MAC, IPv4, mask, gateway, DNS, MTU, DHCP state, lease);
- the ARP cache (16 entries, 60 s);
- the DHCP client state machine and lease timer;
- the DNS query and 8-entry cache;
- UDP sockets, TCP connections and the per-process socket table;
- RX/TX counters and error/drop counters;
- the 100 Hz network thread.

`kernel/src/net/eth.rs` implements Ethernet II, ARP, IPv4 and ICMP.
`kernel/src/net/udp.rs` implements UDP, DHCP and DNS.
`kernel/src/net/tcp.rs` implements the TCP client.

### Threading model

- `net::start()` creates a cooperative kernel thread after the scheduler is
  initialised.
- The thread calls `poll_hardware()` (drain RX, update link) and `run_timers()`
  (DHCP, DNS, TCP, ARP expiry) every 10 ms.
- Blocking syscalls (`NET_PING`, `NET_DNS`, `CONNECT`, `RECV`, `SEND` waiting
  for ARP) also call `net::poll_once()` between `sleep_ms` slices, so progress
  does not depend on the thread being scheduled promptly.
- One `IrqLock<Network>` protects all protocol state; one `IrqLock` inside the
  driver protects the descriptor rings. Lock order is network -> NIC.
- No protocol code runs in interrupt context.

## 4. Protocol behaviour

### Ethernet and ARP

- Ethernet II only; 802.1Q VLAN tags are not parsed.
- Frames are accepted when the destination MAC is ours or broadcast.
- ARP requests are answered for our IPv4 address; replies populate the cache.
- The cache is flat (16 entries), 60 s lifetime, no eviction policy beyond
  replacing the oldest slot when full.

### IPv4

- Version 4, IHL 5-15, total length validated, header checksum validated.
- Fragments are dropped (`flags/fragment offset` nonzero). There is no
  reassembly.
- Destination must be our address or 255.255.255.255.
- TTL 64, DF set, incrementing identification.
- No options are generated; options in received headers are skipped by IHL.
- No IPsec, no source routing, no multicast.

### ICMP

- Echo request is answered; echo reply completes a pending `NET_PING`.
- Ping retries every 400 ms until the caller timeout (100 ms-30 s).
- RTT is reported in whole milliseconds from the 1 kHz uptime clock, so a
  same-host reply is honestly `0 ms`.
- Other ICMP types are ignored.

### UDP

- 8-byte header, payload up to 1472 bytes.
- **Checksum is emitted as zero and not validated on receive.** This is legal
  for IPv4 but weaker than a full implementation; see `KNOWN_LIMITATIONS.md`.
- Destination port dispatch: DHCP (68/67), the active DNS query port, then
  bound UDP sockets. Datagrams for unbound ports are counted as dropped.
- Each UDP socket queues up to 8 datagrams.

### DHCP

- States: 0 idle, 1 selecting, 2 requesting, 3 bound, 4 failed.
- Broadcast DISCOVER, OFFER, REQUEST, ACK/NAK.
- Options parsed: 1 subnet mask, 3 router, 6 DNS, 51 lease, 53 message type,
  54 server identifier.
- Lease renewal is attempted at half the lease with a broadcast REQUEST.
- Retries every 2 s, five attempts, then state 4. A link-down event clears the
  configuration and restarts discovery when the link returns.
- The reference SLIRP lease is `10.0.2.15/24`, gateway `10.0.2.2`, DNS
  `10.0.2.3`, 86400 s.

### DNS

- A records only, recursive-desired queries, UDP port 53.
- Names up to 63 bytes, labels up to 63 bytes.
- Three transmissions 1.5 s apart, then `ETIMEDOUT`.
- Answers are cached for `min(TTL, 300)` seconds, eight entries.
- CNAME chains are not followed; if the first A record is absent the query
  fails with `EIO`.

### TCP

- Client only. There is no `LISTEN`/`ACCEPT`.
- Handshake: SYN, SYN+ACK validation (`ack == iss + 1`), ACK.
- MSS 1460; no MSS option is sent, so 1460 is assumed.
- Send queue 64 KiB, peer window honoured, no congestion window (no slow start
  or congestion avoidance).
- Receive window 128 KiB, advertised as free space. In-order segments are
  accepted fully or partially; out-of-order segments are dropped and
  re-acknowledged.
- One ACK per received data segment; no delayed ACK.
- Retransmission: oldest unacknowledged segment, RTO 300 ms doubling to 3 s,
  five tries, then `ETIMEDOUT`/reset.
- FIN close and RST handling. No keepalive, no SACK, no timestamps, no window
  scaling, no TIME_WAIT timer (closed connections linger for 2 s).
- Checksum includes the IPv4 pseudo-header and is verified on receive.

## 5. Socket API

Socket identifiers are **not** file descriptors. They are allocated by
`SOCKET` and used only with the socket syscalls. File `READ`/`WRITE` cannot be
used on them. Each socket records the owning PID and is released when the
process exits.

| Call | Arguments | Notes |
|---|---|---|
| `SOCKET` | kind (1 UDP, 2 TCP) | returns a positive id |
| `BIND` | id, port (0 = ephemeral) | optional before `CONNECT` |
| `CONNECT` | id, ipv4 pointer, port, timeout | TCP handshake; UDP sets the peer and waits for ARP |
| `SEND` | id, buffer, length | returns bytes accepted |
| `RECV` | id, buffer, length, timeout | returns bytes, `0` on EOF, `-EAGAIN` on timeout |
| `SOCKET_CLOSE` | id | closes and releases the slot |

Ephemeral ports are allocated from `49152 + (socket_id * 7 mod 12000)`.
`SEND` limits one call to 64 KiB. `CONNECT` and `RECV` timeouts are clamped to
0-60 s.

## 6. Configuration

| Call | Effect |
|---|---|
| `NET_CONFIG` mode 0 | enable DHCP and restart discovery |
| `NET_CONFIG` mode 1 | static IPv4, mask, gateway and DNS; disables DHCP |
| `NET_STATUS` | returns `NetStatus` (link, configuration, lease, counters) |

Static configuration is **not persisted** across reboots in 0.4.0. At boot the
stack always starts in DHCP mode. This is stated in Settings.

## 7. Desktop integration

- **Menu bar**: Ethernet icon with state colour and a live tooltip; a click
  opens the Network app.
- **Control Centre**: live Ethernet tile with the current IPv4 address or link
  state; Bluetooth remains honestly "No adapter".
- **Network app**: status card, Renew DHCP, Ping gateway, and a query field
  that accepts an IPv4 address (ping) or a name (DNS).
- **Settings > Network**: adapter, link, method, address, mask, gateway, DNS,
  lease, traffic and a Renew DHCP button.
- **Monitor**: Network card and an `eth0` line with MAC and packet/error
  counters.
- **Terminal**: `net`, `ping <ipv4>` and `dns <name>`.

## 8. Hardware support

Tested:

- QEMU q35 with `-device e1000,netdev=net0` and `-netdev user` (SLIRP).

Supported in principle but **not tested**:

- Physical Intel 82540EM/82545EM/82546 adapters with legacy descriptors.
- QEMU e1000e (`82574L`), igb, virtio-net and RTL8139 are **not** supported.

Real hardware validation requires a machine with a supported NIC, a DHCP
server and a DNS server; it has not been performed. Do not read this milestone
as driver support for arbitrary Ethernet hardware.

## 9. Security properties

- All user pointers are validated by the existing copy helpers.
- Socket ids are checked against the owning process.
- The network stack does not parse untrusted data into kernel pointers;
  lengths are bounded before copies.
- There is no privilege check on `NET_CONFIG`: any process can change the
  interface, and any process can open sockets. There is no firewall.
- The e1000 descriptor rings are kernel-owned and not mapped into user space.

## 10. Verification summary

`tools/net_test.py` boots the system with an explicit e1000 and real host
servers, then runs `NETPROBE.ELF`:

- DHCP lease from SLIRP (`10.0.2.15/24`);
- ICMP echo to the gateway;
- TCP echo of 14 bytes;
- TCP echo of 65,536 bytes with content verification;
- UDP echo;
- DNS A query answered by a host-side server on 127.0.0.1:53.

Exact results and unverified cases are in `TEST_REPORT.md`.
