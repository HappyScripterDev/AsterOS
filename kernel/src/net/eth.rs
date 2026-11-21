//! Ethernet, ARP, IPv4 and ICMP.

use alloc::vec::Vec;

use super::{ArpEntry, Network};

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

pub fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut index = 0;
    while index + 1 < data.len() {
        sum += ((data[index] as u32) << 8) | data[index + 1] as u32;
        index += 2;
    }
    if index < data.len() {
        sum += (data[index] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

fn same_mac(a: &[u8; 6], b: &[u8; 6]) -> bool {
    a == b
}

fn is_broadcast(mac: &[u8; 6]) -> bool {
    mac.iter().all(|byte| *byte == 0xFF)
}

pub fn handle(network: &mut Network, frame: &[u8]) {
    if frame.len() < 14 {
        network.stats.rx_errors += 1;
        return;
    }
    let destination = [frame[0], frame[1], frame[2], frame[3], frame[4], frame[5]];
    if !same_mac(&destination, &network.mac) && !is_broadcast(&destination) {
        return;
    }
    let source = [frame[6], frame[7], frame[8], frame[9], frame[10], frame[11]];
    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    let payload = &frame[14..];
    match ethertype {
        ETHERTYPE_ARP => handle_arp(network, source, payload),
        ETHERTYPE_IPV4 => handle_ipv4(network, source, payload),
        _ => network.stats.dropped += 1,
    }
}

pub fn send(
    network: &mut Network,
    destination: [u8; 6],
    ethertype: u16,
    payload: &[u8],
) -> Result<(), i64> {
    use aster_abi::errno;
    if payload.len() > 1500 {
        return Err(errno::EINVAL);
    }
    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.extend_from_slice(&destination);
    frame.extend_from_slice(&network.mac);
    frame.extend_from_slice(&ethertype.to_be_bytes());
    frame.extend_from_slice(payload);
    match crate::drivers::e1000::send(&frame) {
        Ok(()) => {
            network.stats.tx_packets += 1;
            network.stats.tx_bytes += frame.len() as u64;
            Ok(())
        }
        Err(error) => {
            network.stats.tx_errors += 1;
            Err(error)
        }
    }
}

fn remember_arp(network: &mut Network, ip: [u8; 4], mac: [u8; 6]) {
    let now = crate::time::uptime_ms();
    for entry in &mut network.arp {
        if entry.used && entry.ip == ip {
            entry.mac = mac;
            entry.valid_until = now + 60_000;
            return;
        }
    }
    for entry in &mut network.arp {
        if !entry.used {
            *entry = ArpEntry {
                ip,
                mac,
                valid_until: now + 60_000,
                used: true,
            };
            return;
        }
    }
    // Replace the first entry when the small cache is full.
    network.arp[0] = ArpEntry {
        ip,
        mac,
        valid_until: now + 60_000,
        used: true,
    };
}

pub fn send_arp_request(network: &mut Network, target: [u8; 4]) {
    let mut payload = [0u8; 28];
    payload[0..2].copy_from_slice(&1u16.to_be_bytes());
    payload[2..4].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
    payload[4] = 6;
    payload[5] = 4;
    payload[6..8].copy_from_slice(&1u16.to_be_bytes());
    payload[8..14].copy_from_slice(&network.mac);
    payload[14..18].copy_from_slice(&network.ip);
    payload[18..24].copy_from_slice(&[0; 6]);
    payload[24..28].copy_from_slice(&target);
    let _ = send(network, [0xFF; 6], ETHERTYPE_ARP, &payload);
}

fn send_arp_reply(network: &mut Network, destination_mac: [u8; 6], destination_ip: [u8; 4]) {
    let mut payload = [0u8; 28];
    payload[0..2].copy_from_slice(&1u16.to_be_bytes());
    payload[2..4].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
    payload[4] = 6;
    payload[5] = 4;
    payload[6..8].copy_from_slice(&2u16.to_be_bytes());
    payload[8..14].copy_from_slice(&network.mac);
    payload[14..18].copy_from_slice(&network.ip);
    payload[18..24].copy_from_slice(&destination_mac);
    payload[24..28].copy_from_slice(&destination_ip);
    let _ = send(network, destination_mac, ETHERTYPE_ARP, &payload);
}

fn handle_arp(network: &mut Network, source_mac: [u8; 6], payload: &[u8]) {
    if payload.len() < 28 {
        network.stats.rx_errors += 1;
        return;
    }
    let operation = u16::from_be_bytes([payload[6], payload[7]]);
    let sender_mac = [
        payload[8],
        payload[9],
        payload[10],
        payload[11],
        payload[12],
        payload[13],
    ];
    let sender_ip = [payload[14], payload[15], payload[16], payload[17]];
    let target_ip = [payload[24], payload[25], payload[26], payload[27]];
    if sender_ip != [0; 4] {
        remember_arp(network, sender_ip, sender_mac);
    }
    let _ = source_mac;
    if operation == 1 && target_ip == network.ip {
        send_arp_reply(network, sender_mac, sender_ip);
    }
}

pub fn expire_arp(network: &mut Network, now: u64) {
    for entry in &mut network.arp {
        if entry.used && entry.valid_until <= now {
            *entry = ArpEntry::EMPTY;
        }
    }
}

fn send_ipv4_to_mac(
    network: &mut Network,
    destination_mac: [u8; 6],
    destination: [u8; 4],
    protocol: u8,
    payload: &[u8],
) -> Result<(), i64> {
    use aster_abi::errno;
    if payload.len() + 20 > network.mtu as usize {
        return Err(errno::EINVAL);
    }
    let mut packet = Vec::with_capacity(20 + payload.len());
    packet.push(0x45);
    packet.push(0);
    packet.extend_from_slice(&((20 + payload.len()) as u16).to_be_bytes());
    let id = network.ip_id;
    network.ip_id = network.ip_id.wrapping_add(1);
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&0x4000u16.to_be_bytes());
    packet.push(64);
    packet.push(protocol);
    packet.extend_from_slice(&[0, 0]);
    let source = if network.configured {
        network.ip
    } else {
        [0; 4]
    };
    packet.extend_from_slice(&source);
    packet.extend_from_slice(&destination);
    let header_checksum = checksum(&packet[..20]);
    packet[10..12].copy_from_slice(&header_checksum.to_be_bytes());
    packet.extend_from_slice(payload);
    send(network, destination_mac, ETHERTYPE_IPV4, &packet)
}

pub fn send_ipv4(
    network: &mut Network,
    destination: [u8; 4],
    protocol: u8,
    payload: &[u8],
) -> Result<(), i64> {
    use aster_abi::errno;
    let hop = network.next_hop(destination);
    let Some(mac) = network.arp_lookup(hop) else {
        send_arp_request(network, hop);
        return Err(errno::EAGAIN);
    };
    send_ipv4_to_mac(network, mac, destination, protocol, payload)
}

fn handle_ipv4(network: &mut Network, source_mac: [u8; 6], packet: &[u8]) {
    if packet.len() < 20 || packet[0] >> 4 != 4 {
        network.stats.rx_errors += 1;
        return;
    }
    let header_length = ((packet[0] & 0x0F) as usize) * 4;
    if header_length < 20 || header_length > packet.len() {
        network.stats.rx_errors += 1;
        return;
    }
    let total_length = u16::from_be_bytes([packet[2], packet[3]]) as usize;
    if total_length < header_length || total_length > packet.len() {
        network.stats.rx_errors += 1;
        return;
    }
    if checksum(&packet[..header_length]) != 0 {
        network.stats.rx_errors += 1;
        return;
    }
    let fragment = u16::from_be_bytes([packet[6], packet[7]]);
    if fragment & 0x3FFF != 0 {
        network.stats.dropped += 1;
        return;
    }
    let source = [packet[12], packet[13], packet[14], packet[15]];
    let destination = [packet[16], packet[17], packet[18], packet[19]];
    if destination != network.ip && destination != [255, 255, 255, 255] {
        network.stats.dropped += 1;
        return;
    }
    if source != [0; 4] {
        remember_arp(network, source, source_mac);
    }
    let protocol = packet[9];
    let payload = &packet[header_length..total_length];
    match protocol {
        1 => handle_icmp(network, source, payload),
        17 => super::udp::handle(network, source, payload),
        6 => super::tcp::handle(network, source, payload),
        _ => network.stats.dropped += 1,
    }
}

pub fn send_icmp_echo(
    network: &mut Network,
    destination: [u8; 4],
    destination_mac: [u8; 6],
    id: u16,
    seq: u16,
    data: &[u8],
) {
    let mut icmp = Vec::with_capacity(8 + data.len());
    icmp.push(8);
    icmp.push(0);
    icmp.extend_from_slice(&[0, 0]);
    icmp.extend_from_slice(&id.to_be_bytes());
    icmp.extend_from_slice(&seq.to_be_bytes());
    icmp.extend_from_slice(data);
    let sum = checksum(&icmp);
    icmp[2..4].copy_from_slice(&sum.to_be_bytes());
    let _ = send_ipv4_to_mac(network, destination_mac, destination, 1, &icmp);
}

fn handle_icmp(network: &mut Network, source: [u8; 4], payload: &[u8]) {
    if payload.len() < 8 || checksum(payload) != 0 {
        network.stats.rx_errors += 1;
        return;
    }
    match payload[0] {
        0 => {
            let id = u16::from_be_bytes([payload[4], payload[5]]);
            let seq = u16::from_be_bytes([payload[6], payload[7]]);
            if network.ping.active
                && network.ping.id == id
                && network.ping.seq == seq
                && network.ping.dest == source
            {
                let sent = crate::time::uptime_ms().saturating_sub(network.ping.sent_at);
                network.ping.rtt_ms = sent.min(u32::MAX as u64) as u32;
                network.ping.done = true;
            }
        }
        8 => {
            let mut reply = Vec::with_capacity(payload.len());
            reply.push(0);
            reply.push(0);
            reply.extend_from_slice(&[0, 0]);
            reply.extend_from_slice(&payload[4..]);
            let sum = checksum(&reply);
            reply[2..4].copy_from_slice(&sum.to_be_bytes());
            if let Some(mac) = network.arp_lookup(source) {
                let _ = send_ipv4_to_mac(network, mac, source, 1, &reply);
            }
        }
        _ => {}
    }
}
