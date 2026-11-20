#!/usr/bin/env python3
"""Wallpaper apply probe."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402


def main():
    out = ROOT / "build" / "wallprobe"
    out.mkdir(parents=True, exist_ok=True)
    port = 4474
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:12",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    p = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(8)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
        time.sleep(0.8)
        for ch in "wall":
            qmp.command("send-key", keys=[{"type": "qcode", "data": ch}])
            time.sleep(0.06)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(8.0)
        shot = lambda n: qmp.command("screendump", filename=str((out / n).resolve()))
        shot("01-wallpapers.ppm")
        # Click the second card (Sequoia) inside the window.
        qmp.command("input-send-event", events=[
            {"type": "abs", "data": {"axis": "x", "value": int(700 / 1280 * 32767)}},
            {"type": "abs", "data": {"axis": "y", "value": int(300 / 800 * 32767)}},
        ])
        # PS/2 needs relative motion for the guest; drive from the centre.
        qmp.command("input-send-event", events=[
            {"type": "rel", "data": {"axis": "x", "value": 60}},
            {"type": "rel", "data": {"axis": "y", "value": -45}},
        ])
        time.sleep(0.4)
        qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
        time.sleep(0.08)
        qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
        time.sleep(2.5)
        shot("02-applied.ppm")
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
