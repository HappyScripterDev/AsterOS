#!/usr/bin/env python3
"""Pointer, dock, notifications, Control Centre and brightness test."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402


def move_to(qmp, current, target):
    dx = target[0] - current[0]
    dy = target[1] - current[1]
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
    return target


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.8)


def main() -> None:
    out = ROOT / "build" / "pointer"
    out.mkdir(parents=True, exist_ok=True)
    port = 4465
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:6",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    pos = (640, 400)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        shot("00-desktop.ppm")
        pos = move_to(qmp, pos, (400, 760))
        click(qmp)
        time.sleep(1.0)
        shot("01-terminal.ppm")
        pos = move_to(qmp, pos, (1200, 13))
        click(qmp)
        shot("02-notifications.ppm")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "esc"}])
        time.sleep(0.3)
        pos = move_to(qmp, pos, (1084, 13))
        click(qmp)
        shot("03-control-center.ppm")
        pos = move_to(qmp, pos, (1050, 180))
        click(qmp)
        shot("04-brightness-47.ppm")
        pos = move_to(qmp, pos, (1240, 180))
        click(qmp)
        shot("05-brightness-100.ppm")
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    print("\n".join(sorted(str(p) for p in out.glob("*.ppm"))))


if __name__ == "__main__":
    main()
