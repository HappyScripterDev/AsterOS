#!/usr/bin/env python3
"""Check the Terminal appearance and accent commands."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402


def type_text(qmp, text: str) -> None:
    for ch in text:
        key = "spc" if ch == " " else ch
        qmp.command("send-key", keys=[{"type": "qcode", "data": key}])
        time.sleep(0.06)


def main() -> None:
    out = ROOT / "build" / "accent"
    out.mkdir(parents=True, exist_ok=True)
    port = 4477
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:14",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
        time.sleep(0.8)
        type_text(qmp, "term")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(2.0)
        type_text(qmp, "accent orange")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(1.0)
        type_text(qmp, "accent graphite")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(0.8)
        type_text(qmp, "accent")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(0.8)
        type_text(qmp, "appearance")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(0.8)
        shot("accent.ppm")
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    print("\n".join(str(p) for p in out.glob("*.ppm")))


if __name__ == "__main__":
    main()
