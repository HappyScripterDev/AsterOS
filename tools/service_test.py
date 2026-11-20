#!/usr/bin/env python3
"""Settings service acceptance test: GET/SET, graceful restart, continued save."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402

OUT = ROOT / "build" / "serviceprobe"


def type_text(qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.05)


def enter(qmp):
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4495
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:19",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        time.sleep(1.0)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
        time.sleep(0.8)
        type_text(qmp, "term")
        enter(qmp)
        time.sleep(1.5)

        type_text(qmp, "services")
        enter(qmp)
        time.sleep(0.7)
        type_text(qmp, "accent green")
        enter(qmp)
        time.sleep(1.0)
        qmp.command("screendump", filename=str((OUT / "01-running.ppm").resolve()))

        type_text(qmp, "svcfail")
        enter(qmp)
        time.sleep(2.5)
        type_text(qmp, "services")
        enter(qmp)
        time.sleep(0.7)
        type_text(qmp, "accent orange")
        enter(qmp)
        time.sleep(1.0)
        qmp.command("screendump", filename=str((OUT / "02-restarted.ppm").resolve()))

        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    required = [
        "configd: started, serving settings",
        "service: configd running",
        "configd: settings saved",
        "service: restart requested",
        "configd: exit requested",
    ]
    missing = [line for line in required if line not in text]
    if "configd: running (pid" not in text:
        missing.append("configd: running (pid")
    if text.count("service: configd running") < 2:
        missing.append("second service start")
    if text.count("configd: settings saved") < 2:
        missing.append("save after restart")
    if missing or "KERNEL PANIC" in text or "user fault" in text:
        print("FAIL: missing", missing)
        print("".join(ch for ch in text[-3500:] if ch == "\n" or 32 <= ord(ch) < 127))
        return 1
    print("PASS: configd GET/SET, graceful exit, restart and save after restart")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
