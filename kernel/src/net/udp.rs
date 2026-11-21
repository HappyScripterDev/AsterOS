//! UDP, DHCP client and DNS client.

use alloc::vec::Vec;

use super::{
    eth, DnsCacheEntry, Network, DHCP_BOUND, DHCP_FAILED, DHCP_IDLE, DHCP_REQUESTING,
    DHCP_SELECTING,
};
use crate::sched;

const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_SERVER_PORT: u16 = 67;
const DNS_PORT: u16 = 53;

fn checksum(data: &[u8]) -> u16 {
    eth::checksum(data)
}

pub fn send(
    network: &mut Network,
    source_port: u16,
    destination: [u8; 4],
    destination_port: u16,
    payload: &[u8],
) -> Result<(), i64> {
    use aster_abi::errno;
    if payload.len() > 1472 {
        return Err(errno::EINVAL);
    }
    let mut datagram = Vec::with_capacity(8 + payload.len());
    datagram.extend_from_slice(&source_port.to_be_bytes());
    datagram.extend_from_slice(&destination_port.to_be_bytes());
    datagram.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
    datagram.extend_from_slice(&[0, 0]);
    datagram.extend_from_slice(payload);
    let _ = checksum;
    eth::send_ipv4(network, destination, 17, &datagram)
}

pub fn handle(network: &mut Network, _source: [u8; 4], payload: &[u8]) {
    if payload.len() < 8 {
        network.stats.rx_errors += 1;
        return;
    }
    let source_port = u16::from_be_bytes([payload[0], payload[1]]);
    let destination_port = u16::from_be_bytes([payload[2], payload[3]]);
    let length = u16::from_be_bytes([payload[4], payload[5]]) as usize;
    if length < 8 || length > payload.len() {
        network.stats.rx_errors += 1;
        return;
    }
    let data = &payload[8..length];

    if destination_port == DHCP_CLIENT_PORT || source_port == DHCP_SERVER_PORT {
        dhcp_handle(network, data);
        return;
    }
    if network.dns_query.active && destination_port == network.dns_query.port {
        dns_handle(network, data);
        return;
    }
    for socket in &mut network.udp {
        if socket.active && socket.local_port == destination_port {
            if socket.queue.len() < 8 {
                socket.queue.push(data.to_vec());
            }
            return;
        }
    }
    network.stats.dropped += 1;
}

// ---------------------------------------------------------------------------
// DHCP
// ---------------------------------------------------------------------------

fn dhcp_options(data: &[u8], mut visit: impl FnMut(u8, &[u8])) {
    let mut index = 240;
    while index < data.len() {
        let code = data[index];
        if code == 0 {
            index += 1;
            continue;
        }
        if code == 255 || index + 1 >= data.len() {
            break;
        }
        let length = data[index + 1] as usize;
        if index + 2 + length > data.len() {
            break;
        }
        visit(code, &data[index + 2..index + 2 + length]);
        index += 2 + length;
    }
}

pub fn dhcp_start(network: &mut Network) {
    let xid = (crate::time::uptime_ms() as u32)
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223);
    network.dhcp_xid = xid;
    network.dhcp_state = DHCP_SELECTING;
    network.dhcp_tries = 0;
    network.dhcp_next = 0;
    network.configured = false;
    network.lease_secs = 0;
    send_discover(network);
    network.dhcp_next = crate::time::uptime_ms() + 2000;
    crate::info!("dhcp: discovering");
}

fn dhcp_packet(
    network: &Network,
    message_type: u8,
    requested: Option<[u8; 4]>,
    server: Option<[u8; 4]>,
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(300);
    packet.push(1); // BOOTREQUEST
    packet.push(1); // Ethernet
    packet.push(6); // MAC length
    packet.push(0);
    packet.extend_from_slice(&network.dhcp_xid.to_be_bytes());
    packet.extend_from_slice(&[0, 0]);
    packet.extend_from_slice(&0x8000u16.to_be_bytes()); // broadcast reply
    packet.extend_from_slice(&[0; 4]); // ciaddr
    packet.extend_from_slice(&[0; 4]); // yiaddr
    packet.extend_from_slice(&[0; 4]); // siaddr
    packet.extend_from_slice(&[0; 4]); // giaddr
    packet.extend_from_slice(&network.mac);
    packet.extend_from_slice(&[0; 10]);
    packet.extend_from_slice(&[0; 64]); // sname
    packet.extend_from_slice(&[0; 128]); // file
    packet.extend_from_slice(&[0x63, 0x82, 0x53, 0x63]);
    packet.extend_from_slice(&[53, 1, message_type]);
    if let Some(address) = requested {
        packet.extend_from_slice(&[50, 4]);
        packet.extend_from_slice(&address);
    }
    if let Some(address) = server {
        packet.extend_from_slice(&[54, 4]);
        packet.extend_from_slice(&address);
    }
    packet.extend_from_slice(&[55, 4, 1, 3, 6, 51]);
    packet.extend_from_slice(&[61, 7, 1]);
    packet.extend_from_slice(&network.mac);
    packet.push(255);
    packet
}

fn send_discover(network: &mut Network) {
    let packet = dhcp_packet(network, 1, None, None);
    let _ = send(
        network,
        DHCP_CLIENT_PORT,
        [255; 4],
        DHCP_SERVER_PORT,
        &packet,
    );
}

fn send_request(network: &mut Network) {
    let offered = network.offered_ip;
    let server = network.dhcp_server;
    let packet = dhcp_packet(network, 3, Some(offered), Some(server));
    let _ = send(
        network,
        DHCP_CLIENT_PORT,
        [255; 4],
        DHCP_SERVER_PORT,
        &packet,
    );
}

pub fn dhcp_timer(network: &mut Network, now: u64) {
    if network.dhcp_state == DHCP_BOUND {
        if network.lease_secs > 0 {
            let renew_at = network.lease_start + (network.lease_secs as u64 / 2) * 1000;
            if now >= renew_at {
                network.dhcp_state = DHCP_REQUESTING;
                network.dhcp_tries = 0;
                network.dhcp_next = 0;
                send_request(network);
                crate::info!("dhcp: renewing lease");
            }
            if now >= network.lease_start + network.lease_secs as u64 * 1000 {
                network.configured = false;
                network.dhcp_state = DHCP_FAILED;
                crate::warn!("dhcp: lease expired");
            }
        }
        return;
    }
    if network.dhcp_state != DHCP_SELECTING && network.dhcp_state != DHCP_REQUESTING {
        return;
    }
    if now < network.dhcp_next {
        return;
    }
    if network.dhcp_tries >= 5 {
        network.dhcp_state = DHCP_FAILED;
        network.configured = false;
        crate::warn!("dhcp: no server after retries");
        return;
    }
    network.dhcp_tries += 1;
    network.dhcp_next = now + 2000;
    if network.dhcp_state == DHCP_SELECTING {
        send_discover(network);
    } else {
        send_request(network);
    }
}

fn dhcp_handle(network: &mut Network, data: &[u8]) {
    if data.len() < 240 || data[0] != 2 {
        return;
    }
    let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if xid != network.dhcp_xid {
        return;
    }
    let offered = [data[16], data[17], data[18], data[19]];
    let mut message_type = 0u8;
    let mut subnet = [255, 255, 255, 0];
    let mut router = [0; 4];
    let mut dns = [0; 4];
    let mut lease = 0u32;
    let mut server = [0; 4];
    dhcp_options(data, |code, value| match code {
        1 if value.len() >= 4 => subnet.copy_from_slice(&value[..4]),
        3 if value.len() >= 4 => router.copy_from_slice(&value[..4]),
        6 if value.len() >= 4 => dns.copy_from_slice(&value[..4]),
        51 if value.len() >= 4 => {
            lease = u32::from_be_bytes([value[0], value[1], value[2], value[3]])
        }
        53 if !value.is_empty() => message_type = value[0],
        54 if value.len() >= 4 => server.copy_from_slice(&value[..4]),
        _ => {}
    });

    match message_type {
        2 => {
            if network.dhcp_state != DHCP_SELECTING || offered == [0; 4] || server == [0; 4] {
                return;
            }
            network.offered_ip = offered;
            network.dhcp_server = server;
            network.dhcp_state = DHCP_REQUESTING;
            network.dhcp_tries = 0;
            send_request(network);
            crate::info!(
                "dhcp: offer {} from {}.{}.{}.{}",
                address(offered),
                server[0],
                server[1],
                server[2],
                server[3]
            );
        }
        5 => {
            if network.dhcp_state != DHCP_REQUESTING || offered == [0; 4] {
                return;
            }
            network.ip = offered;
            network.mask = subnet;
            network.gateway = router;
            network.dns = dns;
            network.configured = true;
            network.dhcp_state = DHCP_BOUND;
            network.lease_secs = if lease == 0 { 3600 } else { lease };
            network.lease_start = crate::time::uptime_ms();
            network.clear_arp();
            crate::info!(
                "dhcp: bound {}/{} gateway {} dns {} lease {}s",
                address(network.ip),
                address(network.mask),
                address(network.gateway),
                address(network.dns),
                network.lease_secs
            );
        }
        6 if network.dhcp_state == DHCP_REQUESTING => {
            network.dhcp_state = DHCP_FAILED;
            network.configured = false;
            crate::warn!("dhcp: server rejected the request");
        }
        _ => {}
    }
}

fn address(value: [u8; 4]) -> alloc::string::String {
    alloc::format!("{}.{}.{}.{}", value[0], value[1], value[2], value[3])
}

// ---------------------------------------------------------------------------
// DNS
// ---------------------------------------------------------------------------

fn encode_name(name: &str, out: &mut Vec<u8>) -> bool {
    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    out.len() <= 255
}

pub fn dns_send(network: &mut Network) {
    if !network.dns_query.active {
        return;
    }
    let name = {
        let len = network.dns_query.name_len as usize;
        alloc::string::String::from_utf8_lossy(&network.dns_query.name[..len]).into_owned()
    };
    let mut query = Vec::with_capacity(64);
    query.extend_from_slice(&network.dns_query.id.to_be_bytes());
    query.extend_from_slice(&0x0100u16.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    if !encode_name(&name, &mut query) {
        network.dns_query.error = aster_abi::errno::EINVAL;
        network.dns_query.done = true;
        return;
    }
    query.extend_from_slice(&1u16.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    network.dns_query.sent_at = crate::time::uptime_ms();
    network.dns_query.tries = network.dns_query.tries.saturating_add(1);
    let port = network.dns_query.port;
    let server = network.dns;
    let _ = send(network, port, server, DNS_PORT, &query);
}

pub fn dns_timer(network: &mut Network, now: u64) {
    if !network.dns_query.active || network.dns_query.done {
        return;
    }
    if now.saturating_sub(network.dns_query.sent_at) < 1500 {
        return;
    }
    if network.dns_query.tries >= 3 {
        network.dns_query.error = aster_abi::errno::ETIMEDOUT;
        network.dns_query.done = true;
        return;
    }
    dns_send(network);
}

fn skip_name(data: &[u8], mut index: usize) -> Option<usize> {
    while index < data.len() {
        let length = data[index];
        if length == 0 {
            return Some(index + 1);
        }
        if length & 0xC0 == 0xC0 {
            return if index + 1 < data.len() {
                Some(index + 2)
            } else {
                None
            };
        }
        index += 1 + length as usize;
    }
    None
}

fn dns_handle(network: &mut Network, data: &[u8]) {
    if data.len() < 12 {
        return;
    }
    let id = u16::from_be_bytes([data[0], data[1]]);
    if id != network.dns_query.id {
        return;
    }
    let flags = u16::from_be_bytes([data[2], data[3]]);
    if flags & 0x8000 == 0 {
        return;
    }
    if flags & 0x000F != 0 {
        network.dns_query.error = aster_abi::errno::EIO;
        network.dns_query.done = true;
        return;
    }
    let questions = u16::from_be_bytes([data[4], data[5]]) as usize;
    let answers = u16::from_be_bytes([data[6], data[7]]) as usize;
    let mut index = 12;
    for _ in 0..questions {
        index = match skip_name(data, index) {
            Some(next) => next,
            None => return,
        };
        if index + 4 > data.len() {
            return;
        }
        index += 4;
    }
    for _ in 0..answers {
        index = match skip_name(data, index) {
            Some(next) => next,
            None => return,
        };
        if index + 10 > data.len() {
            return;
        }
        let kind = u16::from_be_bytes([data[index], data[index + 1]]);
        let class = u16::from_be_bytes([data[index + 2], data[index + 3]]);
        let ttl = u32::from_be_bytes([
            data[index + 4],
            data[index + 5],
            data[index + 6],
            data[index + 7],
        ]);
        let length = u16::from_be_bytes([data[index + 8], data[index + 9]]) as usize;
        index += 10;
        if index + length > data.len() {
            return;
        }
        if kind == 1 && class == 1 && length == 4 {
            let ip = [
                data[index],
                data[index + 1],
                data[index + 2],
                data[index + 3],
            ];
            network.dns_query.result = ip;
            network.dns_query.done = true;
            let now = crate::time::uptime_ms();
            let name = network.dns_query.name;
            let name_len = network.dns_query.name_len;
            for entry in &mut network.dns_cache {
                if !entry.used {
                    *entry = DnsCacheEntry {
                        name,
                        name_len,
                        ip,
                        valid_until: now + (ttl.min(300) as u64) * 1000,
                        used: true,
                    };
                    break;
                }
            }
            return;
        }
        index += length;
    }
    network.dns_query.error = aster_abi::errno::EIO;
    network.dns_query.done = true;
}

// Keep the scheduler imported for future blocking helpers in this module.
#[allow(dead_code)]
fn yield_now() {
    sched::yield_now();
}
