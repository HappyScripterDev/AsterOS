#!/usr/bin/env python3
"""Pointer, dock and control-centre interaction test."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402


def rel(qmp, dx, dy):
    qmp.command(
        "input-send-event",
        events=[
            {"type": "rel", "data": {"axis": "x", "value": dx}},
            {"type": "rel", "data": {"axis": "y", "value": dy}},
        ],
    )
    time.sleep(0.06)


def move(qmp, dx, dy, steps=1):
    for _ in range(steps):
        rel(qmp, dx, dy)


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(1.0)


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
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(7)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        shot("0-start.ppm")
        # Centre (640,400) -> Terminal icon (410,760); 40 px per event.
        move(qmp, -40, 0, steps=6)
        move(qmp, 0, 40, steps=9)
        time.sleep(0.6)
        shot("1-pointer-dock.ppm")
        click(qmp)
        time.sleep(1.6)
        shot("2-terminal.ppm")
        # Terminal icon -> control centre icon (1250,14).
        move(qmp, 40, 0, steps=21)
        move(qmp, 0, -40, steps=19)
        time.sleep(0.8)
        shot("3-pointer-top.ppm")
        click(qmp)
        time.sleep(1.0)
        shot("4-control.ppm")
        # Dark mode tile in the control centre (1040,120).
        move(qmp, -40, 0, steps=5)
        move(qmp, 0, 40, steps=3)
        time.sleep(0.4)
        click(qmp)
        time.sleep(2.0)
        shot("5-light-theme.ppm")
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
