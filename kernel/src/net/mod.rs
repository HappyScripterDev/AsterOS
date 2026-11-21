//! Network core: interfaces, protocol dispatch, configuration and timers.
//!
//! The e1000 driver owns the hardware. This module owns interface state, ARP,
//! IPv4, ICMP, UDP/DHCP/DNS, TCP connections and user sockets. A cooperative
//! kernel thread polls the NIC at 100 Hz and runs protocol timers; no protocol
//! runs in interrupt context in this milestone.

pub mod eth;
pub mod tcp;
pub mod udp;

use alloc::vec::Vec;

use crate::drivers::e1000;
use crate::sched;
use crate::sync::IrqLock;

pub const DHCP_IDLE: u8 = 0;
pub const DHCP_SELECTING: u8 = 1;
pub const DHCP_REQUESTING: u8 = 2;
pub const DHCP_BOUND: u8 = 3;
pub const DHCP_FAILED: u8 = 4;

#[derive(Clone, Copy)]
pub struct ArpEntry {
    pub ip: [u8; 4],
    pub mac: [u8; 6],
    pub valid_until: u64,
    pub used: bool,
}

impl ArpEntry {
    pub const EMPTY: Self = Self {
        ip: [0; 4],
        mac: [0; 6],
        valid_until: 0,
        used: false,
    };
}

#[derive(Clone, Copy)]
pub struct PingState {
    pub active: bool,
    pub done: bool,
    pub rtt_ms: u32,
    pub id: u16,
    pub seq: u16,
    pub dest: [u8; 4],
    pub error: i64,
    pub sent_at: u64,
}

impl PingState {
    pub const EMPTY: Self = Self {
        active: false,
        done: false,
        rtt_ms: 0,
        id: 0,
        seq: 0,
        dest: [0; 4],
        error: 0,
        sent_at: 0,
    };
}

#[derive(Clone, Copy)]
pub struct DnsCacheEntry {
    pub name: [u8; 64],
    pub name_len: u8,
    pub ip: [u8; 4],
    pub valid_until: u64,
    pub used: bool,
}

impl DnsCacheEntry {
    pub const EMPTY: Self = Self {
        name: [0; 64],
        name_len: 0,
        ip: [0; 4],
        valid_until: 0,
        used: false,
    };
}

#[derive(Clone, Copy)]
pub struct DnsQuery {
    pub active: bool,
    pub done: bool,
    pub id: u16,
    pub port: u16,
    pub name: [u8; 64],
    pub name_len: u8,
    pub result: [u8; 4],
    pub error: i64,
    pub sent_at: u64,
    pub tries: u8,
}

impl DnsQuery {
    pub const EMPTY: Self = Self {
        active: false,
        done: false,
        id: 0,
        port: 0,
        name: [0; 64],
        name_len: 0,
        result: [0; 4],
        error: 0,
        sent_at: 0,
        tries: 0,
    };
}

#[derive(Clone, Copy, Default)]
pub struct NetStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub dropped: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TcpState {
    Closed,
    SynSent,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    LastAck,
    TimeWait,
    Reset,
}

impl TcpState {
    pub fn code(self) -> u8 {
        match self {
            TcpState::Closed => 0,
            TcpState::SynSent => 1,
            TcpState::Established => 2,
            TcpState::FinWait1 => 3,
            TcpState::FinWait2 => 4,
            TcpState::CloseWait => 5,
            TcpState::LastAck => 6,
            TcpState::TimeWait => 7,
            TcpState::Reset => 8,
        }
    }
}

pub struct TcpSegment {
    pub seq: u32,
    pub data: Vec<u8>,
    pub sent_at: u64,
    pub tries: u8,
    pub fin: bool,
}

pub struct TcpConn {
    pub active: bool,
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,
    pub state: TcpState,
    pub iss: u32,
    pub snd_nxt: u32,
    pub snd_una: u32,
    pub rcv_nxt: u32,
    pub snd_wnd: u16,
    pub send_queue: Vec<u8>,
    pub unacked: Vec<TcpSegment>,
    pub recv_queue: Vec<u8>,
    pub recv_max: usize,
    pub last_activity: u64,
    pub error: i64,
    pub syn_sent_at: u64,
    pub syn_tries: u8,
    pub rto: u64,
    pub fin_pending: bool,
    pub fin_sent: bool,
}

impl TcpConn {
    pub fn new(local_port: u16, remote_ip: [u8; 4], remote_port: u16) -> Self {
        Self {
            active: true,
            local_port,
            remote_ip,
            remote_port,
            state: TcpState::Closed,
            iss: 0,
            snd_nxt: 0,
            snd_una: 0,
            rcv_nxt: 0,
            snd_wnd: 8192,
            send_queue: Vec::new(),
            unacked: Vec::new(),
            recv_queue: Vec::new(),
            recv_max: 131072,
            last_activity: crate::time::uptime_ms(),
            error: 0,
            syn_sent_at: 0,
            syn_tries: 0,
            rto: 300,
            fin_pending: false,
            fin_sent: false,
        }
    }
}

pub struct UdpSocket {
    pub active: bool,
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,
    pub queue: Vec<Vec<u8>>,
}

pub struct Socket {
    pub id: u64,
    pub owner: u64,
    pub kind: u8, // 1 = UDP, 2 = TCP
    pub udp: usize,
    pub tcp: usize,
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,
    pub connected: bool,
}

pub struct Network {
    pub present: bool,
    pub link_up: bool,
    pub mac: [u8; 6],
    pub ip: [u8; 4],
    pub mask: [u8; 4],
    pub gateway: [u8; 4],
    pub dns: [u8; 4],
    pub configured: bool,
    pub mtu: u16,
    pub ip_id: u16,
    pub dhcp_state: u8,
    pub dhcp_enabled: bool,
    pub dhcp_xid: u32,
    pub dhcp_tries: u8,
    pub dhcp_next: u64,
    pub dhcp_server: [u8; 4],
    pub offered_ip: [u8; 4],
    pub lease_secs: u32,
    pub lease_start: u64,
    pub arp: [ArpEntry; 16],
    pub ping: PingState,
    pub dns_query: DnsQuery,
    pub dns_cache: [DnsCacheEntry; 8],
    pub udp: Vec<UdpSocket>,
    pub tcp: Vec<TcpConn>,
    pub sockets: Vec<Socket>,
    pub next_socket: u64,
    pub stats: NetStats,
}

impl Network {
    pub const fn new() -> Self {
        Self {
            present: false,
            link_up: false,
            mac: [0; 6],
            ip: [0; 4],
            mask: [0; 4],
            gateway: [0; 4],
            dns: [0; 4],
            configured: false,
            mtu: 1500,
            ip_id: 1,
            dhcp_state: DHCP_IDLE,
            dhcp_enabled: true,
            dhcp_xid: 0,
            dhcp_tries: 0,
            dhcp_next: 0,
            dhcp_server: [0; 4],
            offered_ip: [0; 4],
            lease_secs: 0,
            lease_start: 0,
            arp: [ArpEntry::EMPTY; 16],
            ping: PingState::EMPTY,
            dns_query: DnsQuery::EMPTY,
            dns_cache: [DnsCacheEntry::EMPTY; 8],
            udp: Vec::new(),
            tcp: Vec::new(),
            sockets: Vec::new(),
            next_socket: 1,
            stats: NetStats {
                rx_packets: 0,
                tx_packets: 0,
                rx_bytes: 0,
                tx_bytes: 0,
                rx_errors: 0,
                tx_errors: 0,
                dropped: 0,
            },
        }
    }

    pub fn next_hop(&self, destination: [u8; 4]) -> [u8; 4] {
        if destination == [255, 255, 255, 255] || destination == self.ip {
            return destination;
        }
        let same_network = (u32::from_be_bytes(destination) & u32::from_be_bytes(self.mask))
            == (u32::from_be_bytes(self.ip) & u32::from_be_bytes(self.mask));
        if same_network {
            destination
        } else {
            self.gateway
        }
    }

    pub fn arp_lookup(&self, ip: [u8; 4]) -> Option<[u8; 6]> {
        if ip == [255, 255, 255, 255] {
            return Some([0xFF; 6]);
        }
        let now = crate::time::uptime_ms();
        self.arp
            .iter()
            .find(|entry| entry.used && entry.ip == ip && entry.valid_until > now)
            .map(|entry| entry.mac)
    }

    pub fn lease_remaining(&self) -> u32 {
        if self.lease_secs == 0 {
            return 0;
        }
        let elapsed = (crate::time::uptime_ms().saturating_sub(self.lease_start) / 1000) as u32;
        self.lease_secs.saturating_sub(elapsed)
    }
}

static NET: IrqLock<Network> = IrqLock::new(Network::new());

pub fn init() {
    if !e1000::present() {
        crate::warn!("net: no supported network adapter");
        return;
    }
    let mac = e1000::mac().unwrap_or([0; 6]);
    let link = e1000::link_up();
    let mut network = NET.lock();
    network.present = true;
    network.link_up = link;
    network.mac = mac;
    crate::info!(
        "net: eth0 {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}, link {}, DHCP enabled",
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5],
        if link { "up" } else { "down" }
    );
    drop(network);
}

pub fn start() {
    if !NET.lock().present {
        return;
    }
    sched::create_kernel_thread("net", network_thread);
    crate::info!("net: protocol thread started");
}

extern "C" fn network_thread() {
    loop {
        poll_hardware();
        run_timers();
        sched::sleep_ms(10);
    }
}

fn poll_hardware() {
    let link = e1000::link_up();
    {
        let mut network = NET.lock();
        if link != network.link_up {
            network.link_up = link;
            crate::info!("net: link {}", if link { "up" } else { "down" });
            if !link {
                network.configured = false;
                network.dhcp_state = DHCP_IDLE;
            } else if network.dhcp_enabled && !network.configured {
                udp::dhcp_start(&mut network);
            }
        }
        if network.link_up
            && network.dhcp_enabled
            && !network.configured
            && network.dhcp_state == DHCP_IDLE
        {
            udp::dhcp_start(&mut network);
        }
    }
    while let Some(frame) = e1000::receive() {
        let mut network = NET.lock();
        network.stats.rx_packets += 1;
        network.stats.rx_bytes += frame.len() as u64;
        eth::handle(&mut network, &frame);
    }
}

fn run_timers() {
    let now = crate::time::uptime_ms();
    let mut network = NET.lock();
    if network.dhcp_enabled && network.link_up && !network.configured {
        udp::dhcp_timer(&mut network, now);
    }
    udp::dns_timer(&mut network, now);
    tcp::timers(&mut network, now);
    eth::expire_arp(&mut network, now);
}

#[derive(Clone, Copy)]
pub struct Status {
    pub present: bool,
    pub link_up: bool,
    pub configured: bool,
    pub dhcp_state: u8,
    pub mac: [u8; 6],
    pub ip: [u8; 4],
    pub mask: [u8; 4],
    pub gateway: [u8; 4],
    pub dns: [u8; 4],
    pub mtu: u16,
    pub lease_remaining: u32,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub dropped: u64,
}

pub fn status() -> Status {
    let network = NET.lock();
    Status {
        present: network.present,
        link_up: network.link_up,
        configured: network.configured,
        dhcp_state: network.dhcp_state,
        mac: network.mac,
        ip: network.ip,
        mask: network.mask,
        gateway: network.gateway,
        dns: network.dns,
        mtu: network.mtu,
        lease_remaining: network.lease_remaining(),
        rx_packets: network.stats.rx_packets,
        tx_packets: network.stats.tx_packets,
        rx_bytes: network.stats.rx_bytes,
        tx_bytes: network.stats.tx_bytes,
        rx_errors: network.stats.rx_errors,
        tx_errors: network.stats.tx_errors,
        dropped: network.stats.dropped,
    }
}

pub fn renew() -> i64 {
    let mut network = NET.lock();
    if !network.present || !network.link_up {
        return aster_abi::errno::EHOSTUNREACH;
    }
    network.dhcp_enabled = true;
    network.configured = false;
    network.dhcp_state = DHCP_SELECTING;
    network.dhcp_tries = 0;
    udp::dhcp_start(&mut network);
    0
}

pub fn set_static(ip: [u8; 4], mask: [u8; 4], gateway: [u8; 4], dns: [u8; 4]) -> i64 {
    if ip == [0; 4] || mask == [0; 4] || ip == [255; 4] {
        return aster_abi::errno::EINVAL;
    }
    let mut network = NET.lock();
    if !network.present {
        return aster_abi::errno::EHOSTUNREACH;
    }
    network.ip = ip;
    network.mask = mask;
    network.gateway = gateway;
    network.dns = dns;
    network.configured = true;
    network.dhcp_enabled = false;
    network.dhcp_state = DHCP_IDLE;
    network.lease_secs = 0;
    network.clear_arp();
    0
}

impl Network {
    pub fn clear_arp(&mut self) {
        for entry in &mut self.arp {
            *entry = ArpEntry::EMPTY;
        }
    }
}

pub fn ping(destination: [u8; 4], timeout_ms: u64) -> Result<u32, i64> {
    use aster_abi::errno;
    {
        let mut network = NET.lock();
        if !network.present || !network.link_up {
            return Err(errno::EHOSTUNREACH);
        }
        if !network.configured {
            return Err(errno::EHOSTUNREACH);
        }
        let id = (crate::time::uptime_ms() as u16) ^ 0xA57E;
        network.ping = PingState {
            active: true,
            done: false,
            rtt_ms: 0,
            id,
            seq: 1,
            dest: destination,
            error: 0,
            sent_at: 0,
        };
    }

    let deadline = crate::time::uptime_ms().saturating_add(timeout_ms);
    let mut last_send = 0u64;
    loop {
        let now = crate::time::uptime_ms();
        let mut network = NET.lock();
        if network.ping.done {
            let rtt = network.ping.rtt_ms;
            network.ping.active = false;
            return Ok(rtt);
        }
        if network.ping.error != 0 {
            let error = network.ping.error;
            network.ping.active = false;
            return Err(error);
        }
        if now >= deadline {
            network.ping.active = false;
            return Err(errno::ETIMEDOUT);
        }
        let hop = network.next_hop(destination);
        if let Some(mac) = network.arp_lookup(hop) {
            if now.saturating_sub(last_send) > 400 || last_send == 0 {
                let id = network.ping.id;
                eth::send_icmp_echo(&mut network, destination, mac, id, 1, b"Aster OS ping");
                network.ping.sent_at = now;
                last_send = now;
            }
        } else {
            eth::send_arp_request(&mut network, hop);
        }
        drop(network);
        poll_once();
        sched::sleep_ms(10);
    }
}

pub fn resolve(name: &str, timeout_ms: u64) -> Result<[u8; 4], i64> {
    use aster_abi::errno;
    if name.is_empty() || name.len() > 63 {
        return Err(errno::EINVAL);
    }
    let now = crate::time::uptime_ms();
    {
        let network = NET.lock();
        for entry in &network.dns_cache {
            if entry.used
                && entry.valid_until > now
                && &entry.name[..entry.name_len as usize] == name.as_bytes()
            {
                return Ok(entry.ip);
            }
        }
    }

    // Make sure the next hop is in the ARP cache before sending the first
    // query; a static reconfiguration clears the cache, and DNS retries over
    // 1.5 s otherwise.
    {
        let mut network = NET.lock();
        if !network.present || !network.link_up || !network.configured {
            return Err(errno::EHOSTUNREACH);
        }
        if network.dns == [0; 4] {
            return Err(errno::EHOSTUNREACH);
        }
        let hop = network.next_hop(network.dns);
        if network.arp_lookup(hop).is_none() {
            eth::send_arp_request(&mut network, hop);
            drop(network);
            let deadline = crate::time::uptime_ms().saturating_add(1000);
            loop {
                poll_once();
                {
                    let network = NET.lock();
                    if network.arp_lookup(hop).is_some() {
                        break;
                    }
                }
                if crate::time::uptime_ms() >= deadline {
                    return Err(errno::EHOSTUNREACH);
                }
                sched::sleep_ms(10);
            }
        }
    }

    {
        let mut network = NET.lock();
        let id = (crate::time::uptime_ms() as u16).wrapping_mul(31) ^ 0x51ED;
        let port = 53000 + (id % 1000);
        let mut query = DnsQuery::EMPTY;
        query.active = true;
        query.id = id;
        query.port = port;
        query.name_len = name.len() as u8;
        query.name[..name.len()].copy_from_slice(name.as_bytes());
        network.dns_query = query;
        udp::dns_send(&mut network);
    }

    let deadline = crate::time::uptime_ms().saturating_add(timeout_ms);
    loop {
        let now = crate::time::uptime_ms();
        let mut network = NET.lock();
        if network.dns_query.done {
            let result = if network.dns_query.error == 0 {
                Ok(network.dns_query.result)
            } else {
                Err(network.dns_query.error)
            };
            network.dns_query.active = false;
            return result;
        }
        if now >= deadline {
            network.dns_query.active = false;
            return Err(errno::ETIMEDOUT);
        }
        drop(network);
        poll_once();
        sched::sleep_ms(10);
    }
}

pub fn close_owner(pid: u64) {
    let mut network = NET.lock();
    let mut tcp_indices = Vec::new();
    let mut udp_indices = Vec::new();
    for socket in &mut network.sockets {
        if socket.owner == pid {
            if socket.kind == 2 {
                tcp_indices.push(socket.tcp);
            } else if socket.kind == 1 {
                udp_indices.push(socket.udp);
            }
            socket.owner = 0;
        }
    }
    for index in tcp_indices {
        tcp::close_connection(&mut network, index);
    }
    for index in udp_indices {
        if index < network.udp.len() {
            network.udp[index].active = false;
            network.udp[index].queue.clear();
        }
    }
    network.sockets.retain(|socket| socket.owner != 0);
}

pub fn tcp_connection_count() -> usize {
    NET.lock().tcp.iter().filter(|conn| conn.active).count()
}

pub fn udp_socket_count() -> usize {
    NET.lock().udp.iter().filter(|socket| socket.active).count()
}

pub fn arp_snapshot(out: &mut [[u8; 4]]) -> usize {
    let network = NET.lock();
    let now = crate::time::uptime_ms();
    let mut count = 0;
    for entry in &network.arp {
        if entry.used && entry.valid_until > now && count < out.len() {
            out[count] = entry.ip;
            count += 1;
        }
    }
    count
}

/// Pump the NIC and protocol timers from a syscall context.
pub fn poll_once() {
    if !NET.lock().present {
        return;
    }
    poll_hardware();
    run_timers();
}

fn free_tcp_slot(network: &mut Network) -> usize {
    if let Some(index) = network.tcp.iter().position(|connection| !connection.active) {
        return index;
    }
    network.tcp.push(TcpConn::new(0, [0; 4], 0));
    network.tcp.len() - 1
}

fn free_udp_slot(network: &mut Network) -> usize {
    if let Some(index) = network.udp.iter().position(|socket| !socket.active) {
        return index;
    }
    network.udp.push(UdpSocket {
        active: false,
        local_port: 0,
        remote_ip: [0; 4],
        remote_port: 0,
        queue: Vec::new(),
    });
    network.udp.len() - 1
}

fn ephemeral_port(network: &Network) -> u16 {
    let base = 49152u32 + (network.next_socket.wrapping_mul(7) % 12000) as u32;
    base as u16
}

pub fn socket_open(owner: u64, kind: u8) -> Result<u64, i64> {
    use aster_abi::errno;
    if kind != 1 && kind != 2 {
        return Err(errno::EINVAL);
    }
    let mut network = NET.lock();
    if !network.present {
        return Err(errno::EHOSTUNREACH);
    }
    if network.sockets.len() >= 32 {
        return Err(errno::ENOMEM);
    }
    let udp = if kind == 1 {
        let index = free_udp_slot(&mut network);
        network.udp[index] = UdpSocket {
            active: true,
            local_port: 0,
            remote_ip: [0; 4],
            remote_port: 0,
            queue: Vec::new(),
        };
        index
    } else {
        usize::MAX
    };
    let tcp = if kind == 2 {
        let index = free_tcp_slot(&mut network);
        network.tcp[index] = TcpConn::new(0, [0; 4], 0);
        index
    } else {
        usize::MAX
    };
    let id = network.next_socket;
    network.next_socket = network.next_socket.wrapping_add(1).max(1);
    network.sockets.push(Socket {
        id,
        owner,
        kind,
        udp,
        tcp,
        local_port: 0,
        remote_ip: [0; 4],
        remote_port: 0,
        connected: false,
    });
    Ok(id)
}

fn socket_index(network: &Network, owner: u64, id: u64) -> Option<usize> {
    network
        .sockets
        .iter()
        .position(|socket| socket.id == id && socket.owner == owner)
}

pub fn socket_bind(owner: u64, id: u64, port: u16) -> Result<(), i64> {
    use aster_abi::errno;
    let mut network = NET.lock();
    let Some(index) = socket_index(&network, owner, id) else {
        return Err(errno::EBADF);
    };
    let port = if port == 0 {
        ephemeral_port(&network)
    } else {
        port
    };
    if port == 0 {
        return Err(errno::EINVAL);
    }
    let kind = network.sockets[index].kind;
    if kind == 1 {
        let udp = network.sockets[index].udp;
        if udp >= network.udp.len() {
            return Err(errno::EBADF);
        }
        if network
            .udp
            .iter()
            .enumerate()
            .any(|(other, socket)| other != udp && socket.active && socket.local_port == port)
        {
            return Err(errno::EADDRINUSE);
        }
        network.udp[udp].local_port = port;
    } else {
        let tcp = network.sockets[index].tcp;
        if tcp >= network.tcp.len() {
            return Err(errno::EBADF);
        }
        if network
            .tcp
            .iter()
            .enumerate()
            .any(|(other, conn)| other != tcp && conn.active && conn.local_port == port)
        {
            return Err(errno::EADDRINUSE);
        }
        network.tcp[tcp].local_port = port;
    }
    network.sockets[index].local_port = port;
    Ok(())
}

pub fn socket_connect(
    owner: u64,
    id: u64,
    remote_ip: [u8; 4],
    remote_port: u16,
    timeout_ms: u64,
) -> Result<(), i64> {
    use aster_abi::errno;
    if remote_port == 0 {
        return Err(errno::EINVAL);
    }
    let ((kind, tcp_index, udp_index), local_port) = {
        let mut network = NET.lock();
        let Some(index) = socket_index(&network, owner, id) else {
            return Err(errno::EBADF);
        };
        if !network.configured {
            return Err(errno::EHOSTUNREACH);
        }
        let kind = network.sockets[index].kind;
        let port = if network.sockets[index].local_port != 0 {
            network.sockets[index].local_port
        } else {
            ephemeral_port(&network)
        };
        network.sockets[index].local_port = port;
        network.sockets[index].remote_ip = remote_ip;
        network.sockets[index].remote_port = remote_port;
        (
            (kind, network.sockets[index].tcp, network.sockets[index].udp),
            port,
        )
    };

    if kind == 1 {
        let mut network = NET.lock();
        if udp_index < network.udp.len() {
            network.udp[udp_index].local_port = local_port;
            network.udp[udp_index].remote_ip = remote_ip;
            network.udp[udp_index].remote_port = remote_port;
        }
        drop(network);
        // Wait for ARP so the first datagram is not lost.
        let deadline = crate::time::uptime_ms().saturating_add(timeout_ms.min(3000));
        loop {
            {
                let network = NET.lock();
                if network.arp_lookup(network.next_hop(remote_ip)).is_some() {
                    break;
                }
            }
            if crate::time::uptime_ms() >= deadline {
                return Err(errno::ETIMEDOUT);
            }
            poll_once();
            sched::sleep_ms(10);
        }
        let mut network = NET.lock();
        let Some(index) = socket_index(&network, owner, id) else {
            return Err(errno::EBADF);
        };
        network.sockets[index].connected = true;
        return Ok(());
    }

    {
        let mut network = NET.lock();
        if tcp_index >= network.tcp.len() {
            return Err(errno::EBADF);
        }
        let iss = (crate::time::uptime_ms() as u32)
            .wrapping_mul(1_103_515_245)
            .wrapping_add(network.next_socket as u32);
        let mut connection = TcpConn::new(local_port, remote_ip, remote_port);
        connection.iss = iss;
        connection.snd_nxt = iss;
        connection.snd_una = iss;
        network.tcp[tcp_index] = connection;
    }
    tcp::connect(tcp_index, timeout_ms)?;
    let mut network = NET.lock();
    let Some(index) = socket_index(&network, owner, id) else {
        return Err(errno::EBADF);
    };
    network.sockets[index].connected = true;
    Ok(())
}

pub fn socket_send(owner: u64, id: u64, data: &[u8]) -> Result<usize, i64> {
    use aster_abi::errno;
    if data.is_empty() {
        return Ok(0);
    }
    if data.len() > 65536 {
        return Err(errno::EINVAL);
    }
    let (kind, index, local_port, remote_ip, remote_port) = {
        let network = NET.lock();
        let Some(position) = socket_index(&network, owner, id) else {
            return Err(errno::EBADF);
        };
        let socket = &network.sockets[position];
        if !socket.connected {
            return Err(errno::ENOTCONN);
        }
        (
            socket.kind,
            if socket.kind == 1 {
                socket.udp
            } else {
                socket.tcp
            },
            socket.local_port,
            socket.remote_ip,
            socket.remote_port,
        )
    };

    if kind == 1 {
        let deadline = crate::time::uptime_ms().saturating_add(2000);
        loop {
            let result = {
                let mut network = NET.lock();
                if index >= network.udp.len() {
                    return Err(errno::EBADF);
                }
                udp::send(&mut network, local_port, remote_ip, remote_port, data)
            };
            match result {
                Ok(()) => return Ok(data.len()),
                Err(errno::EAGAIN) => {
                    if crate::time::uptime_ms() >= deadline {
                        return Err(errno::EHOSTUNREACH);
                    }
                    poll_once();
                    sched::sleep_ms(10);
                }
                Err(error) => return Err(error),
            }
        }
    }

    {
        let mut network = NET.lock();
        if index >= network.tcp.len() || !network.tcp[index].active {
            return Err(errno::EPIPE);
        }
        let connection = &mut network.tcp[index];
        if connection.state != TcpState::Established && connection.state != TcpState::CloseWait {
            return Err(errno::ENOTCONN);
        }
        if connection.send_queue.len() + data.len() > 65536 {
            return Err(errno::ENOSPC);
        }
        connection.send_queue.extend_from_slice(data);
        tcp::flush(&mut network, index);
    }
    Ok(data.len())
}

pub fn socket_recv(owner: u64, id: u64, max: usize, timeout_ms: u64) -> Result<Vec<u8>, i64> {
    use aster_abi::errno;
    if max == 0 {
        return Ok(Vec::new());
    }
    let deadline = crate::time::uptime_ms().saturating_add(timeout_ms);
    loop {
        {
            let mut network = NET.lock();
            let Some(position) = socket_index(&network, owner, id) else {
                return Err(errno::EBADF);
            };
            let socket = &network.sockets[position];
            if socket.kind == 1 {
                let index = socket.udp;
                if index < network.udp.len() {
                    let udp = &mut network.udp[index];
                    if let Some(datagram) = udp.queue.first().cloned() {
                        if datagram.len() <= max {
                            udp.queue.remove(0);
                            return Ok(datagram);
                        }
                        let result = datagram[..max].to_vec();
                        udp.queue[0] = datagram[max..].to_vec();
                        return Ok(result);
                    }
                }
            } else {
                let index = socket.tcp;
                if index < network.tcp.len() {
                    let received = {
                        let connection = &mut network.tcp[index];
                        if !connection.recv_queue.is_empty() {
                            let length = connection.recv_queue.len().min(max);
                            Some(connection.recv_queue.drain(..length).collect::<Vec<u8>>())
                        } else {
                            None
                        }
                    };
                    if let Some(result) = received {
                        tcp::send_ack(&mut network, index);
                        return Ok(result);
                    }
                    let connection = &network.tcp[index];
                    if connection.error != 0 {
                        return Err(connection.error);
                    }
                    if matches!(
                        connection.state,
                        TcpState::CloseWait | TcpState::TimeWait | TcpState::Closed
                    ) {
                        return Ok(Vec::new());
                    }
                    if !connection.active {
                        return Err(errno::EPIPE);
                    }
                }
            }
            if crate::time::uptime_ms() >= deadline {
                return Err(errno::EAGAIN);
            }
        }
        poll_once();
        sched::sleep_ms(5);
    }
}

pub fn socket_close(owner: u64, id: u64) -> i64 {
    let mut network = NET.lock();
    let Some(position) = socket_index(&network, owner, id) else {
        return aster_abi::errno::EBADF;
    };
    let socket = network.sockets.remove(position);
    if socket.kind == 1 && socket.udp < network.udp.len() {
        network.udp[socket.udp].active = false;
        network.udp[socket.udp].queue.clear();
    }
    if socket.kind == 2 {
        tcp::close_connection(&mut network, socket.tcp);
    }
    0
}

pub fn local_port_of(owner: u64, id: u64) -> Option<u16> {
    let network = NET.lock();
    socket_index(&network, owner, id).map(|index| network.sockets[index].local_port)
}
