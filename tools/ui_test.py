#!/usr/bin/env python3
"""Scripted desktop acceptance test.

Boots the image, opens Terminal through the launcher, runs `help`, opens Files,
returns to the launcher, and captures frames. Fails on any kernel panic or user
fault reported over serial.

Pointer-specific behaviour is covered by tools/pointer_test.py; every built-in
app is captured by tools/app_probe.py.
"""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402


def type_text(qmp: Qmp, text: str) -> None:
    for ch in text:
        if ch == " ":
            qmp.command("send-key", keys=[{"type": "qcode", "data": "spc"}])
        elif ch == "/":
            qmp.command("send-key", keys=[{"type": "qcode", "data": "slash"}])
        elif ch == ".":
            qmp.command("send-key", keys=[{"type": "qcode", "data": "dot"}])
        else:
            qmp.command("send-key", keys=[{"type": "qcode", "data": ch}])
        time.sleep(0.05)


def open_launcher(qmp: Qmp) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
    time.sleep(0.8)


def main() -> None:
    out = ROOT / "build" / "ui"
    out.mkdir(parents=True, exist_ok=True)
    port = 4460
    qemu = find_qemu()
    firmware = find_ovmf()
    serial = out / "serial.log"
    serial.write_text("")
    cmd = [
        qemu, "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:1",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    print("starting qemu")
    proc = subprocess.Popen(cmd, cwd=ROOT)
    failures: list[str] = []
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(10)
        shot = lambda name: qmp.command("screendump", filename=str((out / name).resolve()))

        shot("00-desktop.ppm")

        open_launcher(qmp)
        type_text(qmp, "term")
        time.sleep(0.3)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(1.2)
        type_text(qmp, "help")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(1.0)
        shot("01-terminal-help.ppm")

        type_text(qmp, "ls")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(0.8)
        type_text(qmp, "cat /etc/motd")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(0.8)
        shot("01b-terminal-files.ppm")

        open_launcher(qmp)
        type_text(qmp, "set")
        time.sleep(0.3)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(1.5)
        shot("02-settings.ppm")

        open_launcher(qmp)
        type_text(qmp, "fil")
        time.sleep(0.4)
        shot("03-launcher-search.ppm")
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
        time.sleep(1.2)
        shot("04-files.ppm")

        qmp.command("quit")
        qmp.close()
    except Exception as exc:  # noqa: BLE001
        failures.append(str(exc))
        print(f"harness error: {exc}")
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    for marker in ("KERNEL PANIC", "user fault", "PANIC:"):
        if marker in text:
            failures.append(f"serial reported {marker}")
    for name in sorted(out.glob("*.ppm")):
        print(f"screenshot: {name.name} ({name.stat().st_size} bytes)")
    if failures:
        print("UI TEST FAILED:")
        for failure in failures:
            print(f"  - {failure}")
        sys.exit(1)
    print("UI TEST PASSED")


if __name__ == "__main__":
    main()
