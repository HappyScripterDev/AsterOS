# Aster OS 0.4.0 - Roadmap

This milestone adds a real wired network stack to the 0.3.0 system. It keeps
the storage, services, desktop and application behaviour intact.

## Completed in 0.4.0

| Phase | State | Evidence |
|---|---|---|
| Repository audit | DONE | `BASELINE.md`, `v0.3.0-baseline` |
| PCI NIC discovery and MMIO mapping | DONE | `8086:100e` found at 00:01.0, BAR0 mapped |
| Intel 82540EM driver | DONE | EEPROM MAC, link, 256-entry RX ring, 32-entry TX ring, stats |
| Ethernet and ARP | DONE | request/reply, 16-entry cache, 60 s expiry |
| IPv4 and ICMP | DONE | header checksum, echo request/reply, ping syscall |
| UDP | DONE | socket send/receive, DHCP and DNS clients |
| DHCP client | DONE | discover/offer/request/ack, lease, renewal, retries |
| DNS client | DONE | A queries, retries, 8-entry cache, TTL clamp |
| TCP client | DONE | handshake, sliding send window, in-order RX, FIN/RST, retransmit |
| Socket syscalls | DONE | numbers 37-46 with owner and pointer validation |
| Network service thread | DONE | 100 Hz polling and protocol timers |
| Desktop integration | DONE | menu bar, Control Centre, Network app, Settings, Monitor, Terminal |
| QEMU network tests | DONE | DHCP, ICMP, TCP echo, 64 KiB TCP bulk, UDP echo, DNS |
| Documentation | DONE | `NETWORK.md`, `TEST_REPORT.md`, `PERFORMANCE.md`, `IPV6_TLS.md` |

## Next milestones

### 0.5.0 candidates

- Interrupt-driven networking: APIC/IOAPIC, PCI IRQ routing or MSI, then move
  e1000 RX/TX completion off the polling thread.
- Listening TCP sockets (`LISTEN`/`ACCEPT`) and a small in-guest server so the
  network stack can be tested in both directions without a host echo server.
- Out-of-order TCP reassembly, SACK, window scaling, keepalive, and a proper
  TIME_WAIT timer.
- DHCP renewal using the correct unicast REQUEST with `ciaddr`, and lease
  persistence across reboots.
- Persistent network configuration through `configd` (DHCP/static mode, DNS
  override), including a static-address form in Settings.
- A second NIC driver (virtio-net) using the same interface abstraction.
- Network namespaces/capabilities so unprivileged processes cannot reconfigure
  the interface.

### Later

- IPv6 (see `IPV6_TLS.md`), including SLAAC and AAAA records.
- TLS 1.3 with a reviewed crypto implementation and certificate validation.
- DNS over TCP, CNAME following, multiple DNS servers.
- Packet capture and a raw-socket diagnostic interface.
- Hardware validation on a physical machine with a 82540EM-class NIC.
