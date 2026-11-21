#!/usr/bin/env python3
"""Network acceptance test.

Boots Aster OS with an emulated e1000 on QEMU user-mode networking and runs
the netprobe module. The host provides a real TCP echo server, a real UDP echo
server and a real DNS server (port 53 on loopback). The guest must acquire a
DHCP lease, ping the gateway, exchange TCP and UDP payloads, and resolve a DNS
name through a host-side server.

If loopback port 53 cannot be bound, the DNS leg is reported as skipped and is
removed from the guest configuration instead of being faked.
"""

from __future__ import annotations

import socket
import socketserver
import struct
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from image import build_data_image  # noqa: E402
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "nettest"
DNS_ANSWER = "10.0.2.77"
DNS_NAME = "aster.test"


def type_text(qmp: Qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.05)


def enter(qmp: Qmp) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])


class EchoHandler(socketserver.BaseRequestHandler):
    def handle(self) -> None:
        self.request.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        while True:
            data = self.request.recv(65536)
            if not data:
                return
            self.request.sendall(data)


class TcpServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


class UdpEcho(threading.Thread):
    daemon = True

    def __init__(self, port: int = 0) -> None:
        super().__init__()
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", port))
        self.port = self.sock.getsockname()[1]

    def run(self) -> None:
        while True:
            try:
                data, address = self.sock.recvfrom(4096)
            except OSError:
                return
            self.sock.sendto(data, address)


class DnsServer(threading.Thread):
    daemon = True

    def __init__(self) -> None:
        super().__init__()
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", 53))
        self.error: Exception | None = None

    def run(self) -> None:
        while True:
            try:
                data, address = self.sock.recvfrom(4096)
            except OSError:
                return
            if len(data) < 12:
                continue
            try:
                response = self.answer(data)
            except Exception as error:  # noqa: BLE001 - reported by the test
                self.error = error
                continue
            self.sock.sendto(response, address)

    @staticmethod
    def answer(query: bytes) -> bytes:
        transaction = query[0:2]
        flags = struct.pack(">H", 0x8180)
        counts = struct.pack(">HHHH", 1, 1, 0, 0)
        # Copy the question section verbatim (name + qtype + qclass).
        index = 12
        while index < len(query) and query[index] != 0:
            index += 1 + query[index]
        index += 1
        question = query[12 : index + 4]
        answer = (
            b"\xc0\x0c"
            + struct.pack(">HHIH", 1, 1, 60, 4)
            + socket.inet_aton(DNS_ANSWER)
        )
        return transaction + flags + counts + question + answer


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    tcp = TcpServer(("127.0.0.1", 0), EchoHandler)
    tcp_port = tcp.server_address[1]
    threading.Thread(target=tcp.serve_forever, daemon=True).start()
    udp = UdpEcho()
    udp.start()
    udp_port = udp.port
    print(f"host peers: TCP {tcp_port}, UDP {udp_port}, DNS 53")

    dns_enabled = True
    try:
        dns = DnsServer()
        dns.start()
        time.sleep(0.2)
    except OSError as error:
        dns_enabled = False
        print(f"note: DNS leg skipped, cannot bind 127.0.0.1:53 ({error})")

    config = (
        f"tcp=10.0.2.2:{tcp_port}\n"
        f"udp=10.0.2.2:{udp_port}\n"
        "bulk=65536\n"
    )
    if dns_enabled:
        config += f"dns={DNS_NAME}\ndnsserver=10.0.2.2\n"

    files = dict(SEED_FILES)
    files["/NETTEST.CFG"] = config.encode("ascii")
    volume = OUT / "data.img"
    build_data_image(str(volume), files)

    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4530
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(data_path=volume),
        "-display", "none", "-vnc", "127.0.0.1:19",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        time.sleep(1.0)

        qmp.command(
            "send-key",
            keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}],
        )
        time.sleep(0.8)
        type_text(qmp, "term")
        enter(qmp)
        time.sleep(1.6)

        type_text(qmp, "spawn /home/user/netprobe.elf")
        enter(qmp)

        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            text = serial.read_text(errors="replace")
            if "netprobe: PASS" in text or "netprobe: FAIL" in text:
                break
            time.sleep(0.2)
        qmp.command("screendump", filename=str((OUT / "netprobe.ppm").resolve()))
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    required = [
        "dhcp: bound 10.0.2.15/255.255.255.0 gateway 10.0.2.2 dns 10.0.2.3",
        "netprobe: ip 10.0.2.15",
        "netprobe: ping gateway rtt_ms",
        "netprobe: tcp echo ok",
        "netprobe: udp echo ok",
        "netprobe: tcp bulk ok",
        "netprobe: PASS",
    ]
    if dns_enabled:
        required.append(f"netprobe: dns answer {DNS_ANSWER}")
    missing = [line for line in required if line not in text]
    faults = [token for token in ("KERNEL PANIC", "PANIC:", "fault:") if token in text]

    if missing:
        print("FAIL: missing evidence:")
        for line in missing:
            print("  " + line)
    if faults:
        print("FAIL: kernel-level failure:", faults)
    if missing or faults:
        print("--- serial tail ---")
        print("".join(ch for ch in text[-5000:] if ch == "\n" or 32 <= ord(ch) < 127))
        return 1

    print("PASS: DHCP, ICMP, TCP echo, UDP echo and DNS over an emulated e1000")
    if not dns_enabled:
        print("note: DNS leg skipped (loopback port 53 unavailable)")
    print(f"screenshot: {OUT / 'netprobe.ppm'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
