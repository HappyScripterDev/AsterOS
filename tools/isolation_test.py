#!/usr/bin/env python3
"""Process isolation acceptance test.

Spawns a well-behaved child and a faulting child from the Terminal, collects
both exit statuses, and verifies the desktop is still alive afterwards.
"""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402

OUT = ROOT / "build" / "isolation"


def type_text(qmp: Qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.05)


def enter(qmp: Qmp) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4490
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:15",
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
        time.sleep(1.6)

        type_text(qmp, "isolation")
        enter(qmp)
        time.sleep(3.0)
        qmp.command("screendump", filename=str((OUT / "01-isolation.ppm").resolve()))

        # The desktop must still accept input after a child fault.
        type_text(qmp, "ipc")
        enter(qmp)
        time.sleep(2.5)

        type_text(qmp, "ipchup")
        enter(qmp)
        time.sleep(2.5)

        type_text(qmp, "systest")
        enter(qmp)
        time.sleep(1.0)

        type_text(qmp, "uname")
        enter(qmp)
        time.sleep(0.8)
        qmp.command("screendump", filename=str((OUT / "02-after-fault.ppm").resolve()))

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
        "spawn: pid 2 'isoprobe'",
        "isoprobe: running with a private address space",
        "isoprobe: private page verified, exiting with 42",
        "exit: pid 2",
        "spawn: pid 3 'faultprobe'",
        "faultprobe: writing to a kernel address",
        "exit: pid 3",
        "isolation: pid 2 status 42",
        "isolation: pid 3 status -11",
        "pid 3 status -11",
        "ipc: sent 4 bytes, received 4 bytes: pong",
        "ipcprobe: received ping, sending pong",
        "ipc: pid",
        "ipc: forged handle -3",
        "ipc: hangup -14",
        "systest: stat -5 log -5 open -5 wait -5",
        "DESKTOP-READY",
    ]
    missing = [line for line in required if line not in text]
    faults = [token for token in ("KERNEL PANIC", "system halted", "PANIC:") if token in text]

    if missing:
        print("FAIL: missing evidence:")
        for line in missing:
            print("  " + line)
    if faults:
        print("FAIL: kernel-level failure:", faults)
    if "user exception: pid 3" not in text and "user fault: pid 3" not in text:
        print("FAIL: the faulting child was not contained as a user exception")
        missing.append("user exception")

    if missing or faults:
        print("--- serial tail ---")
        print("".join(ch for ch in text[-4000:] if ch == "\n" or 32 <= ord(ch) < 127))
        return 1

    print("PASS: isolated spawn, exit status collection and child fault containment")
    print(f"screenshots: {OUT / '01-isolation.ppm'}, {OUT / '02-after-fault.ppm'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
