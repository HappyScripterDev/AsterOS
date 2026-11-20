#!/usr/bin/env python3
"""Menu bar, Control Centre and About probe."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402


def move_to(qmp, current, target):
    dx = target[0] - current[0]
    dy = target[1] - current[1]
    while dx or dy:
        step_x = max(-120, min(120, dx))
        step_y = max(-120, min(120, dy))
        qmp.command(
            "input-send-event",
            events=[
                {"type": "rel", "data": {"axis": "x", "value": step_x}},
                {"type": "rel", "data": {"axis": "y", "value": step_y}},
            ],
        )
        dx -= step_x
        dy -= step_y
        time.sleep(0.04)
    time.sleep(0.3)
    return target


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.7)


def main():
    out = ROOT / "build" / "shell"
    out.mkdir(parents=True, exist_ok=True)
    port = 4472
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:10",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    p = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        pos = (640, 400)
        shot("00-desktop.ppm")
        pos = move_to(qmp, pos, (60, 13))
        click(qmp)
        shot("01-app-menu.ppm")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "esc"}])
        time.sleep(0.3)
        pos = move_to(qmp, pos, (1084, 13))
        click(qmp)
        shot("02-control-center.ppm")
        pos = move_to(qmp, pos, (1100, 180))
        click(qmp)
        shot("03-brightness.ppm")
        pos = move_to(qmp, pos, (60, 13))
        click(qmp)
        pos = move_to(qmp, pos, (110, 48))
        click(qmp)
        time.sleep(1.5)
        shot("04-about.ppm")
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            p.wait(timeout=6)
        except subprocess.TimeoutExpired:
            p.kill(); p.wait()
    print("\n".join(sorted(str(x) for x in out.glob("*.ppm"))))


if __name__ == "__main__":
    main()
