#!/usr/bin/env python3
"""Open each built-in application through the launcher and capture it."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402

APPS = [
    ("term", "terminal", 1.4),
    ("fil", "files", 1.4),
    ("cal", "calculator", 1.2),
    ("mon", "monitor", 1.6),
    ("tex", "text", 1.2),
    ("not", "notes", 1.6),
    ("cale", "calendar", 1.4),
    ("wal", "wallpapers", 2.4),
    ("set", "settings", 1.8),
]


def type_text(qmp, text: str) -> None:
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": ch}])
        time.sleep(0.05)


def main() -> None:
    out = ROOT / "build" / "apps"
    out.mkdir(parents=True, exist_ok=True)
    qemu = find_qemu()
    port = 4463
    cmd = [
        qemu, "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{out / 'serial.log'}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:4",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(6)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))
        for query, name, wait in APPS:
            qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
            time.sleep(0.7)
            type_text(qmp, query)
            time.sleep(0.3)
            qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
            time.sleep(wait)
            shot(f"{name}.ppm")
            qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "w"}])
            time.sleep(0.5)
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
