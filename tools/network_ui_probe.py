#!/usr/bin/env python3
"""Visual probe for the Network diagnostics app and the Settings network page."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from image import build_data_image  # noqa: E402
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "network-ui"


def type_text(qmp: Qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.04)


def enter(qmp: Qmp) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])


def move_to(qmp: Qmp, current: tuple[int, int], target: tuple[int, int]) -> tuple[int, int]:
    dx = target[0] - current[0]
    dy = target[1] - current[1]
    while dx or dy:
        sx = max(-120, min(120, dx))
        sy = max(-120, min(120, dy))
        qmp.command(
            "input-send-event",
            events=[
                {"type": "rel", "data": {"axis": "x", "value": sx}},
                {"type": "rel", "data": {"axis": "y", "value": sy}},
            ],
        )
        dx -= sx
        dy -= sy
        time.sleep(0.04)
    time.sleep(0.25)
    return target


def click(qmp: Qmp) -> None:
    qmp.command(
        "input-send-event",
        events=[{"type": "btn", "data": {"down": True, "button": "left"}}],
    )
    time.sleep(0.08)
    qmp.command(
        "input-send-event",
        events=[{"type": "btn", "data": {"down": False, "button": "left"}}],
    )
    time.sleep(0.8)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4540
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(data_path=volume),
        "-display", "none", "-vnc", "127.0.0.1:20",
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
        time.sleep(1.2)

        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
        time.sleep(0.7)
        type_text(qmp, "network")
        enter(qmp)
        time.sleep(1.6)
        type_text(qmp, "10.0.2.2")
        enter(qmp)
        time.sleep(1.2)
        qmp.command("screendump", filename=str((OUT / "1-network-ping.ppm").resolve()))

        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "w"}])
        time.sleep(0.5)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
        time.sleep(0.7)
        type_text(qmp, "settings")
        enter(qmp)
        time.sleep(3.5)
        for _ in range(3):
            qmp.command("send-key", keys=[{"type": "qcode", "data": "down"}])
            time.sleep(0.3)
        time.sleep(1.5)
        qmp.command("screendump", filename=str((OUT / "2-settings-network.ppm").resolve()))

        before = serial.read_text(errors="replace").count("dhcp: discovering")
        move_to(qmp, (640, 400), (469, 508))
        click(qmp)
        after = before
        deadline = time.monotonic() + 8.0
        while time.monotonic() < deadline:
            after = serial.read_text(errors="replace").count("dhcp: discovering")
            if after > before:
                break
            time.sleep(0.2)
        qmp.command("screendump", filename=str((OUT / "3-settings-renew.ppm").resolve()))
        time.sleep(1.0)

        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    faults = [token for token in ("KERNEL PANIC", "PANIC:", "user exception") if token in text]
    if faults:
        print("FAIL: kernel or user fault:", faults)
        print(text[-3000:])
        return 1
    if after <= before:
        print("FAIL: the Settings Renew DHCP button did not restart discovery")
        print(text[-3000:])
        return 1
    print(
        "screenshots: "
        f"{OUT / '1-network-ping.ppm'}, {OUT / '2-settings-network.ppm'}, "
        f"{OUT / '3-settings-renew.ppm'}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
