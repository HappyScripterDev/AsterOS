//! TCP transport.
//!
//! A small but real client implementation: three-way handshake, in-order data
//! transfer with a sliding send window, ACK processing, retransmission with a
//! doubling timer, FIN close and RST handling. Out-of-order segments are
//! dropped and re-acknowledged; there is no reassembly queue yet.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use super::{Network, TcpConn, TcpSegment, TcpState};
use crate::sched;

const FLAG_FIN: u8 = 0x01;
const FLAG_SYN: u8 = 0x02;
const FLAG_RST: u8 = 0x04;
const FLAG_PSH: u8 = 0x08;
const FLAG_ACK: u8 = 0x10;

const MSS: usize = 1460;
const DEFAULT_RTO: u64 = 300;
const MAX_TRIES: u8 = 5;

static FIRST_RETRANSMIT: AtomicBool = AtomicBool::new(false);
static FIRST_OUT_OF_ORDER: AtomicBool = AtomicBool::new(false);

fn checksum_with_pseudo(source: [u8; 4], destination: [u8; 4], segment: &[u8]) -> u16 {
    let mut sum = 0u32;
    for pair in [source, destination] {
        sum += ((pair[0] as u32) << 8) | pair[1] as u32;
        sum += ((pair[2] as u32) << 8) | pair[3] as u32;
    }
    sum += 6;
    sum += segment.len() as u32;
    let mut index = 0;
    while index + 1 < segment.len() {
        sum += ((segment[index] as u32) << 8) | segment[index + 1] as u32;
        index += 2;
    }
    if index < segment.len() {
        sum += (segment[index] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

#[allow(clippy::too_many_arguments)]
fn build_segment(
    source_port: u16,
    destination_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut header = alloc::vec![0u8; 20 + payload.len()];
    header[0..2].copy_from_slice(&source_port.to_be_bytes());
    header[2..4].copy_from_slice(&destination_port.to_be_bytes());
    header[4..8].copy_from_slice(&seq.to_be_bytes());
    header[8..12].copy_from_slice(&ack.to_be_bytes());
    header[12] = 5 << 4;
    header[13] = flags;
    header[14..16].copy_from_slice(&window.to_be_bytes());
    header[20..].copy_from_slice(payload);
    header
}

#[allow(clippy::too_many_arguments)]
fn transmit(
    network: &mut Network,
    index: usize,
    seq: u32,
    ack: u32,
    flags: u8,
    window: u16,
    payload: &[u8],
) -> Result<(), i64> {
    let (local_port, remote_port, remote_ip) = {
        let connection = &network.tcp[index];
        (
            connection.local_port,
            connection.remote_port,
            connection.remote_ip,
        )
    };
    let mut packet = build_segment(local_port, remote_port, seq, ack, flags, window, payload);
    let sum = checksum_with_pseudo(network.ip, remote_ip, &packet);
    packet[16..18].copy_from_slice(&sum.to_be_bytes());
    super::eth::send_ipv4(network, remote_ip, 6, &packet)
}

fn advertised_window(connection: &TcpConn) -> u16 {
    let free = connection
        .recv_max
        .saturating_sub(connection.recv_queue.len());
    free.min(65535) as u16
}

pub fn send_ack(network: &mut Network, index: usize) {
    let window = advertised_window(&network.tcp[index]);
    let (snd_nxt, rcv_nxt) = {
        let connection = &network.tcp[index];
        (connection.snd_nxt, connection.rcv_nxt)
    };
    let _ = transmit(network, index, snd_nxt, rcv_nxt, FLAG_ACK, window, &[]);
}

pub fn send_syn(network: &mut Network, index: usize) {
    let window = advertised_window(&network.tcp[index]);
    let (iss, tries) = {
        let connection = &mut network.tcp[index];
        connection.state = TcpState::SynSent;
        connection.syn_sent_at = crate::time::uptime_ms();
        connection.syn_tries = connection.syn_tries.saturating_add(1);
        (connection.iss, connection.syn_tries)
    };
    let _ = tries;
    let _ = transmit(network, index, iss, 0, FLAG_SYN, window, &[]);
}

/// Send any queued application bytes that fit in the peer's window.
pub fn flush(network: &mut Network, index: usize) {
    loop {
        let (state, in_flight, snd_wnd, queued, seq) = {
            let connection = &network.tcp[index];
            (
                connection.state,
                connection.snd_nxt.wrapping_sub(connection.snd_una) as usize,
                connection.snd_wnd as usize,
                connection.send_queue.len(),
                connection.snd_nxt,
            )
        };
        if state != TcpState::Established && state != TcpState::CloseWait {
            return;
        }
        if queued == 0 {
            break;
        }
        let allowed = snd_wnd.saturating_sub(in_flight);
        if allowed == 0 {
            break;
        }
        let length = allowed.min(MSS).min(queued);
        if length == 0 {
            break;
        }
        let payload: Vec<u8> = {
            let connection = &mut network.tcp[index];
            connection.send_queue.drain(..length).collect()
        };
        let window = advertised_window(&network.tcp[index]);
        let rcv_nxt = network.tcp[index].rcv_nxt;
        if transmit(
            network,
            index,
            seq,
            rcv_nxt,
            FLAG_ACK | FLAG_PSH,
            window,
            &payload,
        )
        .is_err()
        {
            let connection = &mut network.tcp[index];
            connection.send_queue.splice(0..0, payload.iter().copied());
            return;
        }
        let now = crate::time::uptime_ms();
        let connection = &mut network.tcp[index];
        connection.snd_nxt = connection.snd_nxt.wrapping_add(length as u32);
        connection.unacked.push(TcpSegment {
            seq,
            data: payload,
            sent_at: now,
            tries: 1,
            fin: false,
        });
    }
    maybe_send_fin(network, index);
}

fn maybe_send_fin(network: &mut Network, index: usize) {
    let (pending, sent, queued, snd_una, snd_nxt) = {
        let connection = &network.tcp[index];
        (
            connection.fin_pending,
            connection.fin_sent,
            connection.send_queue.len(),
            connection.snd_una,
            connection.snd_nxt,
        )
    };
    if !pending || sent || queued != 0 || snd_una != snd_nxt {
        return;
    }
    let window = advertised_window(&network.tcp[index]);
    let seq = snd_nxt;
    let rcv_nxt = network.tcp[index].rcv_nxt;
    let _ = transmit(
        network,
        index,
        seq,
        rcv_nxt,
        FLAG_FIN | FLAG_ACK,
        window,
        &[],
    );
    let connection = &mut network.tcp[index];
    connection.snd_nxt = connection.snd_nxt.wrapping_add(1);
    connection.fin_sent = true;
    connection.unacked.push(TcpSegment {
        seq,
        data: Vec::new(),
        sent_at: crate::time::uptime_ms(),
        tries: 1,
        fin: true,
    });
    connection.state = if connection.state == TcpState::CloseWait {
        TcpState::LastAck
    } else {
        TcpState::FinWait1
    };
}

/// Public close used by the socket layer: queue FIN, do not tear down yet.
pub fn close_connection(network: &mut Network, index: usize) {
    if index >= network.tcp.len() || !network.tcp[index].active {
        return;
    }
    network.tcp[index].fin_pending = true;
    flush(network, index);
    maybe_send_fin(network, index);
}

pub fn abort(network: &mut Network, index: usize) {
    if index >= network.tcp.len() || !network.tcp[index].active {
        return;
    }
    let state = network.tcp[index].state;
    if matches!(
        state,
        TcpState::Established | TcpState::CloseWait | TcpState::FinWait1 | TcpState::FinWait2
    ) {
        let (snd_nxt, rcv_nxt) = {
            let connection = &network.tcp[index];
            (connection.snd_nxt, connection.rcv_nxt)
        };
        let _ = transmit(
            network,
            index,
            snd_nxt,
            rcv_nxt,
            FLAG_RST | FLAG_ACK,
            0,
            &[],
        );
    }
    let connection = &mut network.tcp[index];
    connection.state = TcpState::Reset;
    connection.active = false;
    connection.error = aster_abi::errno::EPIPE;
}

pub fn timers(network: &mut Network, now: u64) {
    for index in 0..network.tcp.len() {
        if !network.tcp[index].active {
            continue;
        }
        let state = network.tcp[index].state;
        if state == TcpState::SynSent {
            let (sent_at, tries, rto) = {
                let connection = &network.tcp[index];
                (connection.syn_sent_at, connection.syn_tries, connection.rto)
            };
            if now.saturating_sub(sent_at) >= rto {
                if tries >= MAX_TRIES {
                    let connection = &mut network.tcp[index];
                    connection.error = aster_abi::errno::ETIMEDOUT;
                    connection.state = TcpState::Reset;
                    connection.active = false;
                } else {
                    network.tcp[index].rto = (rto * 2).min(3000);
                    send_syn(network, index);
                }
            }
            continue;
        }
        // Retransmit the oldest unacknowledged segment.
        let mut resend: Option<(u32, Vec<u8>, bool)> = None;
        {
            let rto = network.tcp[index].rto;
            let connection = &mut network.tcp[index];
            if let Some(seg) = connection.unacked.first_mut() {
                if now.saturating_sub(seg.sent_at) >= rto {
                    if seg.tries >= MAX_TRIES {
                        connection.error = aster_abi::errno::ETIMEDOUT;
                        connection.state = TcpState::Reset;
                        connection.active = false;
                    } else {
                        seg.tries += 1;
                        seg.sent_at = now;
                        if !FIRST_RETRANSMIT.swap(true, Ordering::Relaxed) {
                            crate::info!(
                                "tcp: first retransmit seq {} tries {}",
                                seg.seq,
                                seg.tries
                            );
                        }
                        resend = Some((seg.seq, seg.data.clone(), seg.fin));
                        connection.rto = (connection.rto * 2).min(3000);
                    }
                }
            }
        }
        if let Some((seq, data, fin)) = resend {
            let flags = if fin {
                FLAG_ACK | FLAG_FIN
            } else {
                FLAG_ACK | FLAG_PSH
            };
            let window = advertised_window(&network.tcp[index]);
            let ack = network.tcp[index].rcv_nxt;
            let _ = transmit(network, index, seq, ack, flags, window, &data);
        }
        // Drop fully closed connections after a short linger.
        if matches!(network.tcp[index].state, TcpState::Closed | TcpState::Reset)
            && now.saturating_sub(network.tcp[index].last_activity) > 2000
        {
            network.tcp[index].active = false;
        }
        flush(network, index);
    }
}

pub fn handle(network: &mut Network, source: [u8; 4], payload: &[u8]) {
    if payload.len() < 20 {
        network.stats.rx_errors += 1;
        return;
    }
    let header_length = (((payload[12] >> 4) & 0x0F) as usize) * 4;
    if header_length < 20 || header_length > payload.len() {
        network.stats.rx_errors += 1;
        return;
    }
    if checksum_with_pseudo(source, network.ip, payload) != 0 {
        network.stats.rx_errors += 1;
        return;
    }
    let source_port = u16::from_be_bytes([payload[0], payload[1]]);
    let destination_port = u16::from_be_bytes([payload[2], payload[3]]);
    let seq = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let ack = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let flags = payload[13];
    let window = u16::from_be_bytes([payload[14], payload[15]]);
    let data = &payload[header_length..];

    let index = network.tcp.iter().position(|connection| {
        connection.active
            && connection.local_port == destination_port
            && connection.remote_port == source_port
            && connection.remote_ip == source
    });
    let Some(index) = index else {
        // Refuse unexpected connection attempts to a closed port.
        if flags & FLAG_RST == 0 {
            let mut refusal = build_segment(
                destination_port,
                source_port,
                ack,
                seq.wrapping_add(data.len() as u32 + 1),
                FLAG_RST | FLAG_ACK,
                0,
                &[],
            );
            let sum = checksum_with_pseudo(network.ip, source, &refusal);
            refusal[16..18].copy_from_slice(&sum.to_be_bytes());
            let _ = super::eth::send_ipv4(network, source, 6, &refusal);
        }
        return;
    };

    if flags & FLAG_RST != 0 {
        let connection = &mut network.tcp[index];
        connection.error = aster_abi::errno::ECONNRESET;
        connection.state = TcpState::Reset;
        connection.active = false;
        return;
    }
    network.tcp[index].last_activity = crate::time::uptime_ms();

    if flags & FLAG_SYN != 0 {
        let state = network.tcp[index].state;
        if flags & FLAG_ACK != 0 && state == TcpState::SynSent {
            if ack != network.tcp[index].iss.wrapping_add(1) {
                return;
            }
            {
                let connection = &mut network.tcp[index];
                connection.snd_una = ack;
                connection.snd_nxt = ack;
                connection.rcv_nxt = seq.wrapping_add(1);
                connection.snd_wnd = window;
                connection.state = TcpState::Established;
                connection.rto = DEFAULT_RTO;
                connection.unacked.clear();
            }
            send_ack(network, index);
            flush(network, index);
        } else if flags & FLAG_ACK == 0 {
            send_ack(network, index);
        }
        return;
    }

    // Process the acknowledgement field.
    if flags & FLAG_ACK != 0 {
        let state = network.tcp[index].state;
        let snd_una = network.tcp[index].snd_una;
        if state != TcpState::SynSent && ack.wrapping_sub(snd_una) as i32 > 0 {
            let connection = &mut network.tcp[index];
            connection.snd_una = ack;
            connection.snd_wnd = window;
            connection.rto = DEFAULT_RTO;
            connection.unacked.retain(|seg| {
                seg.seq
                    .wrapping_add(seg.data.len() as u32 + u32::from(seg.fin))
                    .wrapping_sub(ack) as i32
                    > 0
            });
        }
        let (state, snd_nxt, fin_sent) = {
            let connection = &network.tcp[index];
            (connection.state, connection.snd_nxt, connection.fin_sent)
        };
        if fin_sent && ack == snd_nxt {
            let connection = &mut network.tcp[index];
            if state == TcpState::LastAck {
                connection.state = TcpState::Closed;
            } else if state == TcpState::FinWait1 {
                connection.state = TcpState::FinWait2;
            }
        } else if state == TcpState::FinWait1 && ack == snd_nxt {
            network.tcp[index].state = TcpState::FinWait2;
        }
    }

    // Accept in-order payload, partially if the receive window is nearly full.
    if !data.is_empty() {
        let in_order = seq == network.tcp[index].rcv_nxt;
        if !in_order && !FIRST_OUT_OF_ORDER.swap(true, Ordering::Relaxed) {
            crate::info!(
                "tcp: first out-of-order segment seq {} expected {}",
                seq,
                network.tcp[index].rcv_nxt
            );
        }
        let space = network.tcp[index]
            .recv_max
            .saturating_sub(network.tcp[index].recv_queue.len());
        if in_order && space > 0 {
            let accepted = data.len().min(space);
            let connection = &mut network.tcp[index];
            connection.recv_queue.extend_from_slice(&data[..accepted]);
            connection.rcv_nxt = connection.rcv_nxt.wrapping_add(accepted as u32);
        }
        send_ack(network, index);
    }

    // FIN handling.
    if flags & FLAG_FIN != 0 {
        let fin_sequence = seq.wrapping_add(data.len() as u32);
        if fin_sequence == network.tcp[index].rcv_nxt {
            let connection = &mut network.tcp[index];
            connection.rcv_nxt = connection.rcv_nxt.wrapping_add(1);
            connection.state = match connection.state {
                TcpState::Established => TcpState::CloseWait,
                TcpState::FinWait1 | TcpState::FinWait2 => TcpState::TimeWait,
                other => other,
            };
            connection.last_activity = crate::time::uptime_ms();
            send_ack(network, index);
        }
    }

    flush(network, index);
}

/// Blocking connect helper used by the socket layer.
pub fn connect(index: usize, timeout_ms: u64) -> Result<(), i64> {
    let deadline = crate::time::uptime_ms().saturating_add(timeout_ms);
    {
        let mut network = super::NET.lock();
        if index >= network.tcp.len() {
            return Err(aster_abi::errno::EINVAL);
        }
        send_syn(&mut network, index);
    }
    loop {
        sched::sleep_ms(5);
        super::poll_once();
        let mut network = super::NET.lock();
        if index >= network.tcp.len() {
            return Err(aster_abi::errno::EINVAL);
        }
        match network.tcp[index].state {
            TcpState::Established => return Ok(()),
            TcpState::Reset => {
                network.tcp[index].active = false;
                return Err(if network.tcp[index].error != 0 {
                    network.tcp[index].error
                } else {
                    aster_abi::errno::ECONNREFUSED
                });
            }
            _ => {}
        }
        if crate::time::uptime_ms() >= deadline {
            network.tcp[index].active = false;
            network.tcp[index].state = TcpState::Reset;
            network.tcp[index].error = aster_abi::errno::ETIMEDOUT;
            return Err(aster_abi::errno::ETIMEDOUT);
        }
    }
}
