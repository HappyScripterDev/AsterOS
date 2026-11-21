//! Network acceptance probe.
//!
//! Reads `/home/user/NETTEST.CFG`, then exercises the public networking
//! syscalls against the QEMU user-mode network: DHCP status, ICMP echo, TCP
//! echo, UDP echo and DNS. Every result is logged for the host test harness;
//! no result is fabricated.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

const SYS_EXIT: u64 = 1;
const SYS_SLEEP: u64 = 5;
const SYS_TIME_MS: u64 = 6;
const SYS_LOG: u64 = 11;
const SYS_OPEN: u64 = 12;
const SYS_READ: u64 = 13;
const SYS_CLOSE: u64 = 15;
const SYS_NET_STATUS: u64 = 37;
const SYS_NET_CONFIG: u64 = 38;
const SYS_NET_PING: u64 = 39;
const SYS_NET_DNS: u64 = 40;
const SYS_SOCKET: u64 = 41;
const SYS_CONNECT: u64 = 43;
const SYS_SEND: u64 = 44;
const SYS_RECV: u64 = 45;
const SYS_SOCKET_CLOSE: u64 = 46;

const O_RDONLY: u64 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct NetStatus {
    rx_packets: u64,
    tx_packets: u64,
    rx_bytes: u64,
    tx_bytes: u64,
    rx_errors: u64,
    tx_errors: u64,
    dropped: u64,
    mac: [u8; 6],
    present: u8,
    link_up: u8,
    configured: u8,
    dhcp_state: u8,
    ip: [u8; 4],
    mask: [u8; 4],
    gateway: [u8; 4],
    dns: [u8; 4],
    mtu: u32,
    lease_remaining: u32,
}

impl NetStatus {
    const fn empty() -> Self {
        Self {
            rx_packets: 0,
            tx_packets: 0,
            rx_bytes: 0,
            tx_bytes: 0,
            rx_errors: 0,
            tx_errors: 0,
            dropped: 0,
            mac: [0; 6],
            present: 0,
            link_up: 0,
            configured: 0,
            dhcp_state: 0,
            ip: [0; 4],
            mask: [0; 4],
            gateway: [0; 4],
            dns: [0; 4],
            mtu: 0,
            lease_remaining: 0,
        }
    }
}

#[repr(C)]
struct NetConfig {
    mode: u32,
    ip: [u8; 4],
    mask: [u8; 4],
    gateway: [u8; 4],
    dns: [u8; 4],
}

static mut BULK_TX: [u8; 65536] = [0; 65536];
static mut BULK_RX: [u8; 65536] = [0; 65536];

global_asm!(
    r#"
    .section .text.entry, "ax"
    .global _start
_start:
    cld
    xor rbp, rbp
    call probe_main
1:
    hlt
    jmp 1b
"#
);

#[inline]
unsafe fn syscall(number: u64, a: u64, b: u64, c: u64, d: u64) -> i64 {
    let result: i64;
    asm!(
        "int 0x80",
        inlateout("rax") number as i64 => result,
        in("rdi") a,
        in("rsi") b,
        in("rdx") c,
        in("r10") d,
        in("r8") 0u64,
        in("r9") 0u64,
        options(nostack)
    );
    result
}

fn log(message: &[u8]) {
    unsafe {
        syscall(SYS_LOG, message.as_ptr() as u64, message.len() as u64, 0, 0);
    }
}

fn log_line(prefix: &[u8], value: &[u8]) {
    let mut buffer = [0u8; 192];
    let mut length = 0;
    for part in [prefix, value] {
        if length + part.len() >= buffer.len() {
            return;
        }
        buffer[length..length + part.len()].copy_from_slice(part);
        length += part.len();
    }
    buffer[length] = b'\n';
    length += 1;
    log(&buffer[..length]);
}

fn log_u64(prefix: &[u8], value: u64) {
    let mut digits = [0u8; 20];
    let mut index = digits.len();
    let mut number = value;
    loop {
        index -= 1;
        digits[index] = b'0' + (number % 10) as u8;
        number /= 10;
        if number == 0 {
            break;
        }
    }
    log_line(prefix, &digits[index..]);
}

fn log_ip(prefix: &[u8], address: [u8; 4]) {
    let mut text = [0u8; 20];
    let mut length = 0;
    for (part, byte) in address.iter().enumerate() {
        if part > 0 {
            text[length] = b'.';
            length += 1;
        }
        if *byte >= 100 {
            text[length] = b'0' + byte / 100;
            length += 1;
        }
        if *byte >= 10 {
            text[length] = b'0' + (byte / 10) % 10;
            length += 1;
        }
        text[length] = b'0' + byte % 10;
        length += 1;
    }
    log_line(prefix, &text[..length]);
}

fn sleep_ms(milliseconds: u64) {
    unsafe {
        syscall(SYS_SLEEP, milliseconds, 0, 0, 0);
    }
}

fn uptime_ms() -> u64 {
    unsafe { syscall(SYS_TIME_MS, 0, 0, 0, 0) as u64 }
}

fn read_file(path: &str, buffer: &mut [u8]) -> Option<usize> {
    let mut terminated = [0u8; 128];
    if path.len() + 1 > terminated.len() {
        return None;
    }
    terminated[..path.len()].copy_from_slice(path.as_bytes());
    let fd = unsafe { syscall(SYS_OPEN, terminated.as_ptr() as u64, O_RDONLY, 0, 0) };
    if fd < 0 {
        return None;
    }
    let count = unsafe {
        syscall(
            SYS_READ,
            fd as u64,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            0,
        )
    };
    unsafe {
        syscall(SYS_CLOSE, fd as u64, 0, 0, 0);
    }
    if count < 0 {
        None
    } else {
        Some(count as usize)
    }
}

fn parse_ip(text: &str) -> Option<[u8; 4]> {
    let mut address = [0u8; 4];
    let mut part = 0;
    let mut value = 0u16;
    for byte in text.bytes() {
        if byte.is_ascii_digit() {
            value = value * 10 + (byte - b'0') as u16;
        } else if byte == b'.' {
            if part >= 3 || value > 255 {
                return None;
            }
            address[part] = value as u8;
            part += 1;
            value = 0;
        } else {
            return None;
        }
    }
    if part != 3 || value > 255 {
        return None;
    }
    address[3] = value as u8;
    Some(address)
}

fn parse_endpoint(text: &str) -> Option<([u8; 4], u16)> {
    let (address, port) = text.split_once(':')?;
    let ip = parse_ip(address)?;
    let port: u16 = port.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some((ip, port))
}

struct Config {
    tcp: Option<([u8; 4], u16)>,
    udp: Option<([u8; 4], u16)>,
    dns_name: [u8; 64],
    dns_name_len: usize,
    dns_server: Option<[u8; 4]>,
    bulk: usize,
}

fn parse_config(bytes: &[u8]) -> Config {
    let mut config = Config {
        tcp: None,
        udp: None,
        dns_name: [0; 64],
        dns_name_len: 0,
        dns_server: None,
        bulk: 0,
    };
    let text = match core::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return config,
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("tcp=") {
            config.tcp = parse_endpoint(value);
        } else if let Some(value) = line.strip_prefix("udp=") {
            config.udp = parse_endpoint(value);
        } else if let Some(value) = line.strip_prefix("dns=") {
            let bytes = value.as_bytes();
            if bytes.len() <= config.dns_name.len() {
                config.dns_name[..bytes.len()].copy_from_slice(bytes);
                config.dns_name_len = bytes.len();
            }
        } else if let Some(value) = line.strip_prefix("dnsserver=") {
            config.dns_server = parse_ip(value);
        } else if let Some(value) = line.strip_prefix("bulk=") {
            config.bulk = value.parse::<usize>().unwrap_or(0).min(65536);
        }
    }
    config
}

fn read_status() -> NetStatus {
    let mut status = NetStatus::empty();
    unsafe {
        syscall(
            SYS_NET_STATUS,
            &mut status as *mut NetStatus as u64,
            0,
            0,
            0,
        );
    }
    status
}

fn main() -> i32 {
    let mut failures = 0u32;

    let mut status = read_status();
    if status.present == 0 {
        log(b"netprobe: no network adapter\n");
        return 2;
    }
    log_ip(b"netprobe: ip ", status.ip);
    log_ip(b"netprobe: mask ", status.mask);
    log_ip(b"netprobe: gateway ", status.gateway);
    log_ip(b"netprobe: dns ", status.dns);
    log_u64(b"netprobe: mtu ", status.mtu as u64);

    let deadline = uptime_ms() + 8000;
    while status.configured == 0 && uptime_ms() < deadline {
        sleep_ms(50);
        status = read_status();
    }
    if status.configured == 0 {
        log(b"netprobe: DHCP did not configure the interface\n");
        return 3;
    }

    let mut message = [0u8; 512];
    let Some(length) = read_file("/home/user/NETTEST.CFG", &mut message) else {
        log(b"netprobe: no NETTEST.CFG, status only\n");
        return 0;
    };
    let config = parse_config(&message[..length]);

    // ICMP echo to the gateway.
    let mut rtt = 0u32;
    let result = unsafe {
        syscall(
            SYS_NET_PING,
            status.gateway.as_ptr() as u64,
            2000,
            &mut rtt as *mut u32 as u64,
            0,
        )
    };
    if result < 0 {
        log_u64(b"netprobe: ping failed ", (-result) as u64);
        failures += 1;
    } else {
        log_u64(b"netprobe: ping gateway rtt_ms ", rtt as u64);
    }

    if let Some((address, port)) = config.tcp {
        if run_tcp(address, port) {
            log(b"netprobe: tcp echo ok\n");
        } else {
            log(b"netprobe: tcp echo failed\n");
            failures += 1;
        }
    }

    if config.bulk > 0 {
        if let Some((address, port)) = config.tcp {
            if run_tcp_bulk(address, port, config.bulk) {
                log(b"netprobe: tcp bulk ok\n");
            } else {
                log(b"netprobe: tcp bulk failed\n");
                failures += 1;
            }
        }
    }

    if let Some((address, port)) = config.udp {
        if run_udp(address, port) {
            log(b"netprobe: udp echo ok\n");
        } else {
            log(b"netprobe: udp echo failed\n");
            failures += 1;
        }
    }

    if config.dns_name_len != 0 {
        if let Some(server) = config.dns_server {
            let mut static_config = NetConfig {
                mode: 1,
                ip: status.ip,
                mask: status.mask,
                gateway: status.gateway,
                dns: server,
            };
            let result = unsafe {
                syscall(
                    SYS_NET_CONFIG,
                    &mut static_config as *mut NetConfig as u64,
                    0,
                    0,
                    0,
                )
            };
            if result < 0 {
                log(b"netprobe: dns server configuration failed\n");
                failures += 1;
            } else {
                let mut answer = [0u8; 4];
                let query_started = uptime_ms();
                let result = unsafe {
                    syscall(
                        SYS_NET_DNS,
                        config.dns_name.as_ptr() as u64,
                        config.dns_name_len as u64,
                        answer.as_mut_ptr() as u64,
                        4000,
                    )
                };
                if result < 0 {
                    log_u64(b"netprobe: dns failed ", (-result) as u64);
                    failures += 1;
                } else {
                    log_ip(b"netprobe: dns answer ", answer);
                    log_u64(
                        b"netprobe: dns ms ",
                        uptime_ms().saturating_sub(query_started),
                    );
                }
            }
        } else {
            log(b"netprobe: dns name configured without a server\n");
        }
    }

    if failures == 0 {
        log(b"netprobe: PASS\n");
        0
    } else {
        log_u64(b"netprobe: FAIL count ", failures as u64);
        failures as i32
    }
}

fn run_tcp(address: [u8; 4], port: u16) -> bool {
    let id = unsafe { syscall(SYS_SOCKET, 2, 0, 0, 0) };
    if id <= 0 {
        log_u64(b"netprobe: socket tcp error ", (-id) as u64);
        return false;
    }
    let result = unsafe {
        syscall(
            SYS_CONNECT,
            id as u64,
            address.as_ptr() as u64,
            port as u64,
            5000,
        )
    };
    if result < 0 {
        log_u64(b"netprobe: tcp connect error ", (-result) as u64);
        unsafe {
            syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
        }
        return false;
    }
    let payload = b"aster tcp echo\n";
    let sent = unsafe {
        syscall(
            SYS_SEND,
            id as u64,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
        )
    };
    if sent != payload.len() as i64 {
        log_u64(b"netprobe: tcp send error ", (-sent) as u64);
        unsafe {
            syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
        }
        return false;
    }
    let mut reply = [0u8; 64];
    let received = unsafe {
        syscall(
            SYS_RECV,
            id as u64,
            reply.as_mut_ptr() as u64,
            reply.len() as u64,
            5000,
        )
    };
    unsafe {
        syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
    }
    if received != payload.len() as i64 {
        log_u64(b"netprobe: tcp recv error ", (-received) as u64);
        return false;
    }
    &reply[..received as usize] == payload
}

fn run_tcp_bulk(address: [u8; 4], port: u16, size: usize) -> bool {
    let id = unsafe { syscall(SYS_SOCKET, 2, 0, 0, 0) };
    if id <= 0 {
        log_u64(b"netprobe: bulk socket error ", (-id) as u64);
        return false;
    }
    let result = unsafe {
        syscall(
            SYS_CONNECT,
            id as u64,
            address.as_ptr() as u64,
            port as u64,
            5000,
        )
    };
    if result < 0 {
        log_u64(b"netprobe: bulk connect error ", (-result) as u64);
        unsafe {
            syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
        }
        return false;
    }

    let size = size.min(65536);
    let tx = unsafe { &mut *core::ptr::addr_of_mut!(BULK_TX) };
    let rx = unsafe { &mut *core::ptr::addr_of_mut!(BULK_RX) };
    for index in 0..size {
        tx[index] = (index as u32).wrapping_mul(31).wrapping_add(7) as u8;
    }

    let started = uptime_ms();
    let sent = unsafe { syscall(SYS_SEND, id as u64, tx.as_ptr() as u64, size as u64, 0) };
    if sent != size as i64 {
        log_u64(b"netprobe: bulk send error ", (-sent) as u64);
        unsafe {
            syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
        }
        return false;
    }

    let deadline = started + 20000;
    let mut received = 0usize;
    let mut first_byte_at = 0u64;
    let mut chunks = 0u64;
    let mut max_gap = 0u64;
    let mut last_at = started;
    while received < size {
        if uptime_ms() > deadline {
            log_u64(b"netprobe: bulk timeout at ", received as u64);
            unsafe {
                syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
            }
            return false;
        }
        let want = (size - received).min(8192);
        let count = unsafe {
            syscall(
                SYS_RECV,
                id as u64,
                rx.as_mut_ptr().add(received) as u64,
                want as u64,
                3000,
            )
        };
        if count <= 0 {
            log_u64(b"netprobe: bulk recv error ", (-count) as u64);
            unsafe {
                syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
            }
            return false;
        }
        received += count as usize;
        let now = uptime_ms();
        if first_byte_at == 0 {
            first_byte_at = now;
            log_u64(
                b"netprobe: bulk first byte ms ",
                now.saturating_sub(started),
            );
        }
        chunks += 1;
        max_gap = max_gap.max(now.saturating_sub(last_at));
        last_at = now;
    }
    let elapsed = uptime_ms().saturating_sub(started);
    log_u64(b"netprobe: bulk chunks ", chunks);
    log_u64(b"netprobe: bulk max gap ms ", max_gap);
    unsafe {
        syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
    }
    if tx[..size] != rx[..size] {
        log(b"netprobe: bulk payload mismatch\n");
        return false;
    }
    log_u64(b"netprobe: tcp bulk bytes ", size as u64);
    log_u64(b"netprobe: tcp bulk ms ", elapsed);
    true
}

fn run_udp(address: [u8; 4], port: u16) -> bool {
    let id = unsafe { syscall(SYS_SOCKET, 1, 0, 0, 0) };
    if id <= 0 {
        log_u64(b"netprobe: socket udp error ", (-id) as u64);
        return false;
    }
    let result = unsafe {
        syscall(
            SYS_CONNECT,
            id as u64,
            address.as_ptr() as u64,
            port as u64,
            4000,
        )
    };
    if result < 0 {
        log_u64(b"netprobe: udp connect error ", (-result) as u64);
        unsafe {
            syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
        }
        return false;
    }
    let payload = b"aster udp echo\n";
    let sent = unsafe {
        syscall(
            SYS_SEND,
            id as u64,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
        )
    };
    if sent != payload.len() as i64 {
        log_u64(b"netprobe: udp send error ", (-sent) as u64);
        unsafe {
            syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
        }
        return false;
    }
    let mut reply = [0u8; 64];
    let received = unsafe {
        syscall(
            SYS_RECV,
            id as u64,
            reply.as_mut_ptr() as u64,
            reply.len() as u64,
            4000,
        )
    };
    unsafe {
        syscall(SYS_SOCKET_CLOSE, id as u64, 0, 0, 0);
    }
    if received != payload.len() as i64 {
        log_u64(b"netprobe: udp recv error ", (-received) as u64);
        return false;
    }
    &reply[..received as usize] == payload
}

#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    let code = main();
    unsafe {
        syscall(SYS_EXIT, code as u64, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    log(b"netprobe: panic\n");
    unsafe {
        syscall(SYS_EXIT, 99, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}
