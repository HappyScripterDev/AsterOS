# Aster OS 0.4.0 - Known Limitations

This is the honest list of what Aster OS 0.4.0 does **not** do, or does with
weaker guarantees than a production operating system. Everything in this list
is either visible in the code or was absent from the test plan; nothing here is
silently presented as finished in the UI.

## Networking

- **One NIC model only.** The driver supports Intel 82540EM (`8086:100e`), which
  is what QEMU exposes in the tests. e1000e, igb, virtio-net, RTL8139 and real
  server adapters are not supported.
- **No physical hardware validation.** All networking evidence comes from QEMU
  user-mode networking. A physical 82540EM-class adapter, a real switch and a
  real DHCP/DNS server have not been tested.
- **Polling, not interrupts.** There is no APIC/IOAPIC or PCI IRQ routing, so
  the NIC IRQ is masked. A kernel thread polls every 10 ms. This adds up to
  10 ms of latency and consumes CPU while idle.
- **No link-state events.** Link up/down is sampled by polling; a cable event
  during a transfer is not signalled promptly.
- **No IPv6 and no TLS.** Only architecture notes exist (`IPV6_TLS.md`). No
  partial or fake protocol code is shipped.
- **No firewall, NAT, routing daemon or packet filter.** Any process can open
  sockets and any process can call `NET_CONFIG`.
- **Single interface, single IPv4 address, single default gateway.** No
  secondary addresses, policy routing, VLANs or interface aliases.
- **MTU fixed at 1500.** Jumbo frames and path-MTU discovery are not
  implemented.
- **No multicast and no broadcast receive API.** Broadcast is handled only for
  ARP/DHCP internally.
- **Static configuration is not persistent.** Every boot starts with DHCP.
  `NET_CONFIG` can switch to static at runtime, but the choice is lost on
  reboot. Settings has no static-address form yet.

### Ethernet, ARP, IPv4, ICMP

- Ethernet II only; 802.1Q tags are not parsed.
- ARP cache is a flat 16-entry table with a 60 s lifetime; no eviction policy
  beyond replacing the first slot when full, no gratuitous ARP, no ARP
  conflict detection.
- IPv4 fragments are dropped; there is no reassembly. Large UDP datagrams that
  would require fragmentation fail instead.
- No IP options, no source routing, no IPsec.
- ICMP only answers echo requests and matches echo replies. Destination
  unreachable, time exceeded and redirect messages are ignored.
- Ping reports whole milliseconds, so local replies show `0 ms` rather than a
  sub-millisecond figure.

### UDP, DHCP, DNS

- **UDP checksums are sent as zero and are not validated on receive.** This is
  legal for IPv4 but provides weaker integrity than a full implementation.
- UDP socket receive queue holds at most 8 datagrams; excess datagrams are
  dropped.
- DHCP supports one interface, a broadcast renew at half-lease, five retries
  and no persistence. It does not implement the full unicast renewal state
  (`ciaddr`), rebinding, hostname, client class or classless static routes.
- DNS resolves **A records only**. No AAAA, CNAME following, MX, SRV, TXT,
  DNSSEC or DNS-over-TCP.
- DNS names are limited to 63 bytes and 8 cached answers; the cache TTL is
  clamped to 300 s. Query retries are fixed at 1.5 s intervals.

### TCP and sockets

- **Client only.** `LISTEN` and `ACCEPT` do not exist, so the system cannot
  host a TCP service.
- **No out-of-order reassembly.** Segments that arrive ahead of `rcv_nxt` are
  dropped and re-acknowledged; the peer must retransmit. The implementation
  accepts in-order data fully or partially, but has no reassembly queue.
- No SACK, timestamps, window scaling, keepalive, urgent data or TCP Fast Open.
- No congestion control (no slow start, congestion avoidance or fast
  retransmit). Sending is limited only by the peer's advertised window and the
  64 KiB send queue.
- Retransmission covers only the oldest unacknowledged segment, with a 300 ms
  initial RTO, doubling to 3 s for five tries.
- MSS is assumed to be 1460; the MSS option is neither sent nor parsed.
- No `TIME_WAIT` timer; connections linger for 2 s and are then marked
  inactive.
- Socket calls are blocking with a timeout. There is no `select`/`poll`/epoll`,
  non-blocking mode, `shutdown`, socket options, half-close API or `MSG_PEEK`.
- One `SEND` call accepts at most 64 KiB. The system caps 32 live sockets.
- Socket ids are not file descriptors; `READ`/`WRITE` cannot be used on them.
  There is no `accept`-style descriptor handoff yet.

## Configuration and UI

- Settings > Network shows live state and can renew DHCP. It cannot edit a
  static address; that is only reachable through `NET_CONFIG` from a program.
- The Network app performs ping/DNS synchronously; its 2-2.5 s timeout can
  stall desktop compositing during the request.
- Monitor shows current counters, not network history graphs or per-socket
  state.
- Menu bar and Control Centre show a single Ethernet interface. A missing NIC
  is reported honestly as "No wired adapter"; no status is fabricated.

## Preserved limitations from 0.3.0

- Built-in applications still share the desktop address space. Only spawned
  processes (for example `netprobe`) have private page tables.
- FAT32 uses 8.3 names, has no journal and no permissions.
- No `KILL`; no per-process memory quotas.
- No ASLR, SMEP, SMAP or UMIP.
- No SMP; one CPU.
- IPC messages are 256 bytes inline; no shared memory or handle passing.
- No user accounts, authentication, package manager, installer, updater or
  browser.
- No audio, USB or GPU acceleration.

## Test gaps

The network test does not cover:

- physical hardware;
- induced packet loss or corruption (so retransmission is implemented but not
  stress-verified);
- DHCP renewal across a full lease;
- link-down/reconnect during traffic;
- inbound TCP (no server exists);
- DNS NXDOMAIN/timeout paths;
- IPv6 or TLS.
