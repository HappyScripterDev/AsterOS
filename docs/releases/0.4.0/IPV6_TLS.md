# Aster OS 0.4.0 - IPv6 and TLS Architecture (design only)

**Status: design notes. No IPv6 or TLS code is implemented in 0.4.0.** This
document exists so the next milestone does not have to redesign the socket and
packet layers, and so no unsafe shortcut is mistaken for a finished feature.

## 1. IPv6

### 1.1 Goals

- Dual-stack IPv4 + IPv6 on the same interface.
- Link-local addresses, SLAAC from router advertisements, optional DHCPv6.
- Neighbor Discovery replacing ARP.
- ICMPv6 echo, packet-too-big and destination-unreachable handling.
- The existing socket API stays valid; only the address family changes.

### 1.2 Data model

```text
Interface
  mac, mtu, link state
  ipv4: address, mask, gateway, dns
  ipv6: [Ipv6Address; 4]
        each { addr: [u8;16], prefix_len: u8, state, valid_until, preferred_until }
  ipv6_routers: [Router; 4] { addr, lifetime, preference }
  neighbor_cache: [Neighbor; 32]
        { addr: [u8;16], mac: [u8;6], state, reachable_until, retries }
  dad: pending address probes
```

`Ipv6Address` is a 16-byte value type. Addresses are stored in network byte
order. The current `[u8; 4]` IPv4 fields remain; the interface carries both
families rather than a union, which keeps IPv4 code paths unchanged.

### 1.3 Packet path

- EtherType `0x86DD`.
- Fixed 40-byte header: version/traffic class/flow label, payload length, next
  header, hop limit, source, destination.
- Extension-header walker with a strict hop limit (for example 8 headers) and
  bounds checks: hop-by-hop, routing, fragment, destination options, AH/ESP are
  rejected unless explicitly supported.
- No router fragmentation. The source fragments at 1280 bytes or larger when
  needed; the current stack would instead return `EMSGSIZE` until a fragment
  sender exists.
- Minimum MTU 1280. The Ethernet MTU remains 1500; path MTU discovery uses
  ICMPv6 Packet Too Big and an MTU cache per destination.
- Multicast addresses: `ff02::1` (all nodes), `ff02::2` (all routers),
  solicited-node `ff02::1:ffXX:XXXX`. The NIC must accept the corresponding
  multicast MACs (`33:33:...`).

### 1.4 Neighbor Discovery and SLAAC

- ICMPv6 types: 133 Router Solicitation, 134 Router Advertisement,
  135 Neighbor Solicitation, 136 Neighbor Advertisement, 128/129 echo.
- DAD: tentative address, join solicited-node multicast, send NS with
  unspecified source, wait 1 s; a defending NA rejects the address.
- SLAAC: take the /64 prefix from RA Prefix Information, form the interface
  identifier. Prefer stable-privacy (RFC 7217) over EUI-64; EUI-64 exposes the
  MAC. Stable-privacy needs a persistent secret, which does not exist yet.
  Until it does, document EUI-64 as a privacy limitation.
- Router lifetime and prefix valid/preferred lifetimes are timers on the
  network thread, like DHCP.
- DHCPv6 (stateful) is optional and later; SLAAC is the first target.

### 1.5 Address selection and API

- Add an address-family field to `NetConfig`, `NetStatus`, `NET_PING` and
  `NET_DNS`. Because the ABI is additive, new calls or a versioned structure
  are preferred over changing the existing IPv4 structures.
- Proposed additions (numbers 47+): `NET_STATUS6`, `NET_DNS6`,
  `NET_PING6`, `SOCKET6` or a family argument on `SOCKET`.
- Socket addresses become an enum internally:

```text
SockAddr::V4([u8;4], u16)
SockAddr::V6([u8;16], u16, scope_id)
```

- DNS: AAAA records alongside A. The current `dns_query` stores one answer;
  it becomes a small result set with separate A/AAAA slots.

### 1.6 Dual-stack rules (initial, deliberately simple)

1. Prefer IPv6 when the destination resolves to AAAA and a global/ULA address
   exists with a working router.
2. Fall back to IPv4 if IPv6 connection setup times out, but only before any
   application data is sent.
3. Link-local destinations require a scope id; no implicit interface guessing.
4. No NAT64, no DNS64, no transition tunnels.

### 1.7 Testing plan

- QEMU user networking with IPv6 enabled, host-side services bound to `::1`.
- Static link-local and global addresses first; SLAAC with a host RA daemon
  second.
- ICMPv6 echo, DNS AAAA, TCP over IPv6, neighbor unreachability.
- No physical IPv6 router test until hardware validation exists.

## 2. TLS 1.3

### 2.1 Where it lives

TLS belongs in **user space**, above the socket syscalls, not in the kernel:

- cryptography stays out of ring 0;
- the kernel ABI does not grow a TLS state machine;
- a compromised TLS library cannot corrupt kernel memory;
- the same library can serve the desktop, a future browser and tools.

The kernel only needs to provide reliable byte streams and entropy.

### 2.2 Prerequisites that do not exist yet

| Prerequisite | Current state |
|---|---|
| CSPRNG | **Missing.** No entropy pool, no `/dev/urandom`, no `getrandom` syscall |
| SHA-256 / HMAC / HKDF | Missing |
| AES-128-GCM or ChaCha20-Poly1305 | Missing |
| X25519 | Missing |
| Ed25519 or ECDSA P-256 | Missing |
| X.509 parser | Missing |
| Trust store | Missing |
| Reasonably correct clock | RTC exists, but no authenticated time source |

A `getrandom` syscall backed by a seeded ChaCha20 or SHA-256 DRBG is the first
deliverable. Until it exists, **no TLS code may be written**, because a
predictable RNG silently breaks every guarantee.

### 2.3 Protocol profile

- TLS 1.3 only (RFC 8446). No SSLv3, TLS 1.0/1.1/1.2 fallback.
- Cipher suites: `TLS_AES_128_GCM_SHA256` and
  `TLS_CHACHA20_POLY1305_SHA256` when a reviewed implementation is available.
- Key exchange: X25519; P-256 as a later option.
- Authentication: Ed25519 and ECDSA P-256 certificates. No RSA key exchange.
- SNI sent; no 0-RTT in the first version (replay risk).
- Certificate validation: chain to a configured trust store, validity window,
  SAN matching, basic constraints, key usage, no wildcard beyond one label.
  Revocation (OCSP/CRL) is explicitly out of scope initially and documented.
- No silent plaintext fallback. A failed handshake returns an error; callers
  must opt in to insecure mode explicitly if such a mode ever exists.

### 2.4 API shape

```text
TlsClient::connect(tcp_socket, server_name, trust_store)
  -> TlsStream

TlsStream::write(&[u8]) -> usize
TlsStream::read(&mut [u8]) -> usize
TlsStream::close()
```

Record layer: 16 KiB plaintext records, 5-byte header, sequence numbers,
AEAD nonce construction, `close_notify` handling. Rekeying and KeyUpdate are
required for long-lived connections; session resumption (PSK) is a later
optimisation.

### 2.5 Verification

- Known-answer tests for every primitive (NIST CAVP / RFC 7748 / RFC 8439).
- RFC 8448 handshake traces.
- Interoperability against a host TLS 1.3 server and a public test endpoint
  only after the trust store and clock exist.
- Negative tests: bad MAC, truncated record, wrong certificate name, expired
  certificate, downgrade attempt, replayed 0-RTT.
- Constant-time review for secret-dependent branches and table lookups.

### 2.6 Explicit non-goals for 0.4.0

No TLS client, no certificate store, no `getrandom`, no HTTPS app. The only
deliverable in this release is this architecture and the statement that none of
it is implemented.
