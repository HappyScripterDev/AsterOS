#!/usr/bin/env python3
"""Window drag and virtual desktop probe."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402


def rel(qmp, dx, dy, steps=1):
    for _ in range(steps):
        qmp.command(
            "input-send-event",
            events=[
                {"type": "rel", "data": {"axis": "x", "value": dx}},
                {"type": "rel", "data": {"axis": "y", "value": dy}},
            ],
        )
        time.sleep(0.05)


def down(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.1)


def up(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.8)


def main() -> None:
    out = ROOT / "build" / "wm"
    out.mkdir(parents=True, exist_ok=True)
    port = 4466
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:7",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(6)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        # Open Terminal from the dock.
        rel(qmp, -40, 0, 6)
        rel(qmp, 0, 40, 9)
        down(qmp)
        up(qmp)
        time.sleep(1.0)
        shot("0-terminal.ppm")
        # Drag the titlebar: move to (600,80), press, move +120/+60.
        rel(qmp, 40, 0, 5)
        rel(qmp, 0, -40, 17)
        down(qmp)
        rel(qmp, 40, 0, 3)
        rel(qmp, 0, 40, 1)
        up(qmp)
        shot("1-dragged.ppm")
        # Switch desktop right and back.
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "right"}])
        time.sleep(0.8)
        shot("2-desktop2.ppm")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "left"}])
        time.sleep(0.8)
        shot("3-back.ppm")
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
