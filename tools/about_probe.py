#!/usr/bin/env python3
"""About page + wallpaper application probe."""

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
        dx -= sx; dy -= sy
        time.sleep(0.04)
    time.sleep(0.3)
    return target


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.8)


def key(qmp, *names):
    for name in names:
        qmp.command("send-key", keys=[{"type": "qcode", "data": name}])
        time.sleep(0.06)


def main():
    out = ROOT / "build" / "about"
    out.mkdir(parents=True, exist_ok=True)
    port = 4473
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:11",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    p = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(8)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        pos = (640, 400)
        pos = move_to(qmp, pos, (60, 13))
        click(qmp)  # open App menu
        pos = move_to(qmp, pos, (110, 48))
        click(qmp)  # About Aster OS
        time.sleep(1.6)
        shot("01-about.ppm")
        key(qmp, "ctrl", "w")
        time.sleep(0.4)
        key(qmp, "ctrl", "spc")
        time.sleep(0.8)
        for ch in "wal":
            key(qmp, ch)
        key(qmp, "ret")
        time.sleep(2.2)
        shot("02-wallpapers.ppm")
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
