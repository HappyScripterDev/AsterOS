# Aster OS 0.4.0 - Test Report

Date: 2026-11-21
Branch: `milestone/aster-0.4.0`
Reference machine: QEMU q35, 1 vCPU, 512 MiB, OVMF, 1280x800, e1000 on
QEMU user-mode networking (SLIRP), Windows host.

## 1. What was run

Every result below comes from a booted system driven by QMP, or from a host
test that parses the produced image. No result was inferred from code.

| Test | Result | Evidence |
|---|---|---|
| `python tools/test.py` | PASS | host unit tests, build, boot smoke, e1000 + DHCP markers, no faults |
| `python tools/net_test.py` | PASS | DHCP, ICMP, TCP echo, 64 KiB TCP bulk echo, UDP echo, DNS |
| `python tools/network_ui_probe.py` | PASS | Network app ping result and Settings network page screenshots |
| `python tools/ui_test.py` | PASS | desktop, Terminal, Settings, Files, Spotlight |
| `python tools/app_probe.py` | PASS | 10 apps including the new Network app |
| `python tools/pointer_test.py` | PASS | dock, notifications, Control Centre, brightness |
| `python tools/wm_probe.py` | PASS | drag, virtual desktops |
| `python tools/files_probe.py` | PASS | create, rename, copy, paste, delete, search, info |
| `python tools/fs_probe.py` | PASS | 64 KiB write/read, rename, delete, sync |
| `python tools/persistence_test.py` | PASS | guest write, host FAT verifier, reboot, read-back |
| `python tools/storage_probe.py` | PASS | capacity, missing disk, corrupt volume |
| `python tools/isolation_test.py` | PASS | status 42, fault -11, IPC pong, forged handle, hangup |
| `python tools/settings_test.py` | PASS | corruption, validation, valid round-trip |
| `python tools/service_test.py` | PASS | GET/SET, exit, restart, save after restart |
| `python tools/final_ui_probe.py` | PASS | menus, About, Control Centre, wallpaper |
| `python tools/status_probe.py` | PASS | Monitor log view, Settings storage |
| `python tools/shell_probe.py`, `about_probe.py`, `accent_probe.py` | PASS | shell and appearance probes |

## 2. Exact network evidence

Serial output from `tools/net_test.py`:

```
[     0.010] info: e1000: 52:54:00:12:34:56, link up (ctrl 0x140260, status 0x80080783)
[     0.010] info: net: eth0 52:54:00:12:34:56, link up, DHCP enabled
[     0.010] info: net: protocol thread started
[     0.010] info: e1000: first TX completed (301 bytes)
[     0.010] info: dhcp: discovering
[     0.690] info: e1000: first RX (590 bytes)
[     0.690] info: dhcp: offer 10.0.2.15 from 10.0.2.2
[     0.702] info: dhcp: bound 10.0.2.15/255.255.255.0 gateway 10.0.2.2 dns 10.0.2.3 lease 86400s
[user] netprobe: ip 10.0.2.15
[user] netprobe: mask 255.255.255.0
[user] netprobe: gateway 10.0.2.2
[user] netprobe: dns 10.0.2.3
[user] netprobe: mtu 1500
[user] netprobe: ping gateway rtt_ms 0
[user] netprobe: tcp echo ok
[user] netprobe: bulk first byte ms 12
[user] netprobe: bulk chunks 8
[user] netprobe: bulk max gap ms 12
[user] netprobe: tcp bulk bytes 65536
[user] netprobe: tcp bulk ms 12
[user] netprobe: tcp bulk ok
[user] netprobe: udp echo ok
[user] netprobe: dns answer 10.0.2.77
[user] netprobe: dns ms 10
[user] netprobe: PASS
```

The host-side peers were:

- TCP echo on `127.0.0.1:9000` (`TCP_NODELAY`, 64 KiB reads);
- UDP echo on `127.0.0.1:9001`;
- DNS server on `127.0.0.1:53`, answering `aster.test` with `10.0.2.77`;
- QEMU SLIRP mapping `10.0.2.2` to the host loopback and providing the DHCP
  lease.

The 64 KiB TCP leg sends a deterministic 65,536-byte pattern, receives the
echo, and compares every byte before reporting `tcp bulk ok`.

## 3. Desktop evidence

`build/network-ui/1-network-ping.ppm` shows the Network app with live data
(`Intel 82540EM`, link up, DHCP bound, `10.0.2.15`, gateway `10.0.2.2`, DNS
`10.0.2.3`, MAC, traffic, lease) and the result `ping 10.0.2.2: reply in 0 ms`.

`build/network-ui/2-settings-network.ppm` shows Settings > Network with the
same live values and a Renew DHCP button.

`build/apps/monitor.ppm` shows the Monitor Network card (`10.0.2.15`) and the
`eth0` line with MAC and RX/TX counters.

## 4. Timing observed

| Measurement | Value |
|---|---|
| DHCP bound after kernel entry | 0.702 s |
| ICMP echo RTT to 10.0.2.2 | 0 ms (below 1 ms clock resolution) |
| TCP 14-byte echo | immediate |
| TCP 65,536-byte echo round trip | 11-12 ms |
| DNS query to host-local server | 10 ms |
| Boot to first compositor frame | 3.468 s (this run) |
| Idle compositor frame | 22.6 ms average |
| Free RAM after boot | 442 MiB |

## 5. Not tested

These are explicit gaps, not claims:

- **No physical hardware test.** Only QEMU's 82540EM was exercised.
- **No interrupt path.** The NIC is polled; no MSI/IOAPIC test exists because
  the feature is not implemented.
- **No IPv6 and no TLS.** Only architecture documents exist.
- **No TCP server.** `LISTEN`/`ACCEPT` do not exist, so no inbound connection
  test was possible.
- **No forced packet loss or corruption test.** Retransmission code exists but
  was not exercised by an induced-loss harness.
- **No DHCP renewal over a full lease.** The renewal timer is implemented but
  the test lease is 86400 s, so only the initial bind was observed.
- **No link-down/reconnect test.** QEMU link state was not toggled.
- **No second NIC, jumbo frames, MTU below 1500, VLAN, multicast or raw
  sockets.**
- **No DNS timeout/NXDOMAIN test.** The successful A-record path was tested.
- **No UDP checksum validation.** The stack sends zero checksums and accepts
  datagrams without verifying the checksum.

The honest summary is: the implemented paths above work end-to-end in QEMU;
everything else is unimplemented or untested.
