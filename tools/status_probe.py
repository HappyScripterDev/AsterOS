#!/usr/bin/env python3
"""Capture Monitor storage/logs and Settings storage pages."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402

OUT = ROOT / "build" / "statusprobe"


def move(qmp, dx, dy):
    while dx or dy:
        sx = max(-120, min(120, dx))
        sy = max(-120, min(120, dy))
        qmp.command("input-send-event", events=[
            {"type": "rel", "data": {"axis": "x", "value": sx}},
            {"type": "rel", "data": {"axis": "y", "value": sy}},
        ])
        dx -= sx
        dy -= sy
        time.sleep(0.04)
    time.sleep(0.25)


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.9)


def type_text(qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.05)


def enter(qmp):
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])


def open_app(qmp, query: str) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
    time.sleep(0.8)
    type_text(qmp, query)
    enter(qmp)
    time.sleep(1.8)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4494
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:18",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    pos = (640, 400)
    try:
        qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        time.sleep(1.0)

        open_app(qmp, "mon")
        qmp.command("screendump", filename=str((OUT / "01-monitor.ppm").resolve()))

        move(qmp, 338 - pos[0], 384 - pos[1])
        pos = (338, 384)
        click(qmp)
        time.sleep(0.6)
        qmp.command("screendump", filename=str((OUT / "02-logs.ppm").resolve()))

        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "w"}])
        time.sleep(0.7)
        open_app(qmp, "set")
        move(qmp, 250 - pos[0], 337 - pos[1])
        pos = (250, 337)
        click(qmp)
        qmp.command("screendump", filename=str((OUT / "03-storage.ppm").resolve()))

        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    if "KERNEL PANIC" in text or "user fault" in text:
        print("FAIL: fault during status probe")
        return 1
    print("PASS: status probe captured")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
