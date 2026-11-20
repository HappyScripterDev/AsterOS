#!/usr/bin/env python3
"""Reproducible boot and desktop benchmark for Aster OS.

Measures wall-clock time from QEMU launch to serial markers, then reports the
compositor frame time and memory figures from the kernel log.
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, IMAGE  # noqa: E402

MARKERS = [
    ("BOOT-LOADER-START", "firmware handed control to Aster Boot"),
    ("BOOT-HANDOFF-OK", "kernel image loaded, ExitBootServices done"),
    ("KERNEL-ENTRY-OK", "kernel entry (serial up, before subsystem init)"),
    ("KERNEL-READY", "kernel initialised"),
    ("DESKTOP-READY", "desktop shell initialised"),
    ("frame 1", "first compositor frame"),
]


def main() -> None:
    out = ROOT / "build" / "bench"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    serial.write_text("")
    port = 4467
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        "-display", "none", "-vnc", "127.0.0.1:8",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    start = time.monotonic()
    proc = subprocess.Popen(cmd, cwd=ROOT)
    results: dict[str, float | str] = {}
    try:
        qmp = Qmp(port, timeout=20)
        seen: dict[str, float] = {}
        deadline = start + 30
        while time.monotonic() < deadline and len(seen) < len(MARKERS):
            text = serial.read_text(errors="replace")
            for marker, _ in MARKERS:
                if marker not in seen and marker in text:
                    seen[marker] = round(time.monotonic() - start, 3)
            time.sleep(0.02)
        time.sleep(8.0)
        qmp.command("quit")
        qmp.close()
        results.update({marker: seen.get(marker) for marker, _ in MARKERS})
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    for line in text.splitlines():
        if "memory:" in line or "frame 1 at" in line or "frames," in line:
            results.setdefault("log", "")
            results["log"] += line.strip() + "\n"
    print("Aster OS benchmark")
    print("------------------")
    for marker, description in MARKERS:
        value = results.get(marker)
        print(f"{description:42s} {value if value is not None else 'not seen'} s")
    for line in str(results.get("log", "")).splitlines():
        print(line)
    (out / "result.json").write_text(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
