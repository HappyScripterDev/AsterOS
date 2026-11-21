# Aster OS 0.4.0 - Performance

Reference machine: QEMU q35, 1 vCPU, 512 MiB, OVMF, std VGA 1280x800, e1000
on QEMU user-mode networking, TCG (no KVM), Windows host.

## Boot and desktop

Measured with `tools/bench.py` on a fresh data volume:

| Marker | Wall time |
|---|---|
| Firmware hands control to Aster Boot | 1.551 s |
| Kernel image loaded, `ExitBootServices` done | 1.764 s |
| Kernel entry | 1.764 s |
| Kernel initialised (`KERNEL-READY`) | 1.813 s |
| Desktop shell initialised (`DESKTOP-READY`) | 3.400 s |
| First compositor frame | 3.468 s |

- Idle compositor: **22.6 ms** average at frame 120.
- Free RAM after boot: **442 MiB** (0.3.0 recorded 446 MiB).
- Network driver memory: roughly **1.2 MiB** for 256 RX buffers, 32 TX
  buffers, two descriptor rings and bookkeeping.
- Network initialisation adds no measurable delay to `KERNEL-READY`; DHCP runs
  asynchronously and completed at **0.702 s** after kernel entry in the network
  test.

The first-frame number varies by several hundred milliseconds between TCG runs
and between host load conditions; treat it as a range, not a precise constant.

## Protocol latency

| Operation | Measured | Notes |
|---|---|---|
| DHCP bind | 0.70 s after kernel entry | broadcast DISCOVER/OFFER/REQUEST/ACK over SLIRP |
| ICMP echo to 10.0.2.2 | 0 ms | below the 1 ms uptime clock resolution |
| TCP 65,536-byte echo round trip | 11-12 ms | 8 receive chunks, max inter-chunk gap 12 ms |
| DNS A query (host-local server) | 10 ms | includes the reply and cache insert |

TCP bulk calculation: 65,536 bytes each way in 0.012 s is approximately
**5.2-5.5 MiB/s per direction** and **10.4-11 MiB/s aggregate** for the echo
round trip. This is a loopback/SLIRP measurement with a Python peer, not a
physical wire rate. The stack has no congestion window, so this number says
more about the host path than about achievable WAN throughput.

## Polling cost

The network thread wakes every 10 ms and drains the RX ring. In the idle case
this is one timer tick plus a small number of barrier reads; there is no
interrupt storm because NIC interrupts are masked. A formal CPU-utilisation
profile of the thread was not recorded. With a polling design, packet latency
has a floor of one poll interval (up to 10 ms); the measured TCP and DNS
latencies are consistent with that floor.

## Notes from development

- The first TX implementation polled the wrong descriptor byte. With the
  correct legacy layout (`cmd` at 11, `status` at 12) and no redundant wait on
  an already-free descriptor, the 64 KiB echo improved from ~1.2 s to ~12 ms.
  The final driver does not wait before filling a free descriptor and still
  waits for DD after submission.
- RX uses 256 descriptors. A 32-descriptor ring could not absorb a 64 KiB echo
  burst on this polling schedule and produced retransmission stalls.
- DNS queries after a static reconfiguration now resolve ARP before the first
  query; this removed a 1.5 s retry delay and brought the query to 10 ms.

## What is not measured

- Physical 100 Mbit/s or 1 Gbit/s hardware.
- Throughput with packet loss, small windows, or a remote (non-loopback)
  peer.
- CPU time consumed by the network thread over a long idle period.
- Power consumption (no power management exists).
