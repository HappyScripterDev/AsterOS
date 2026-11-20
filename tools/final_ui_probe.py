#!/usr/bin/env python3
"""Validate Control Centre layering and applying a wallpaper."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402


def move(qmp, dx, dy):
    while dx or dy:
        sx = max(-120, min(120, dx))
        sy = max(-120, min(120, dy))
        qmp.command("input-send-event", events=[
            {"type": "rel", "data": {"axis": "x", "value": sx}},
            {"type": "rel", "data": {"axis": "y", "value": sy}},
        ])
        dx -= sx; dy -= sy
        time.sleep(0.04)
    time.sleep(0.25)


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.9)


def main():
    out = ROOT / "build" / "finalui"
    out.mkdir(parents=True, exist_ok=True)
    port = 4475
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:13",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    p = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9)
        shot = lambda n: qmp.command("screendump", filename=str((out / n).resolve()))
        # Control centre.
        move(qmp, 444, -387)  # 640,400 -> 1084,13
        click(qmp)
        shot("01-control-center.ppm")
        time.sleep(0.2)
        # Close it.
        move(qmp, -400, 300)
        click(qmp)
        time.sleep(0.3)
        # Spotlight -> Wallpapers.
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
        time.sleep(0.8)
        for ch in "wall":
            qmp.command("send-key", keys=[{"type": "qcode", "data": ch}])
            time.sleep(0.06)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(4.0)
        shot("02-wallpapers.ppm")
        # Move to the Sequoia card and click it.
        move(qmp, 60, -170)
        click(qmp)
        time.sleep(2.5)
        shot("03-sequoia.ppm")
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
