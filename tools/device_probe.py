#!/usr/bin/env python3
"""Device Manager / hardware inventory probe.

Runs the Terminal `devices` command, verifies the real PCI inventory and opens
Settings > Devices for a screenshot. No fake USB/audio/thermal rows exist.
"""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from image import build_data_image  # noqa: E402
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "devices"


def key(qmp: Qmp, code: str, mods: tuple[str, ...] = ()) -> None:
    keys = [{"type": "qcode", "data": value} for value in mods]
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)


def type_text(qmp: Qmp, text: str) -> None:
    for ch in text:
        key(qmp, {" ": "spc"}.get(ch, ch))
        time.sleep(0.1)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4580
    proc = subprocess.Popen(
        [
            find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
            "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
            "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
            "-serial", f"file:{serial}", "-monitor", "none",
            *drive_args(data_path=volume),
            "-display", "none", "-vnc", "127.0.0.1:49",
            "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
        ],
        cwd=ROOT,
    )
    qmp = None
    failures: list[str] = []
    try:
        qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        time.sleep(1.2)
        key(qmp, "spc", ("ctrl",))
        time.sleep(3.0)
        type_text(qmp, "term")
        key(qmp, "ret")
        time.sleep(6.0)
        type_text(qmp, "devices")
        key(qmp, "ret")
        time.sleep(3.0)
        text = serial.read_text(errors="replace")
        required = [
            "pci: 00:02.0 8086:100e",
            "class 02/00",
            "irq 11",
            "driver",
            "pci: 00:1f.2 8086:2922",
            "class 01/06",
            "pci: 00:01.0",
            "class 03/00",
        ]
        for marker in required:
            if marker not in text:
                failures.append(f"missing device evidence: {marker}")
        if "PANIC" in text:
            failures.append("panic while enumerating devices")

        key(qmp, "spc", ("ctrl",))
        time.sleep(2.5)
        type_text(qmp, "settings")
        key(qmp, "ret")
        time.sleep(5.0)
        for _ in range(11):
            key(qmp, "down")
            time.sleep(0.15)
        time.sleep(2.0)
        qmp.command("screendump", filename=str((OUT / "devices.ppm").resolve()))

        if failures:
            print("FAIL")
            for failure in failures:
                print(f"  {failure}")
            return 1
        print("PASS: real PCI inventory and Settings > Devices")
        return 0
    finally:
        if qmp is not None:
            try:
                qmp.command("quit")
            except Exception:
                pass
            try:
                qmp.close()
            except Exception:
                pass
        if proc.poll() is None:
            proc.kill()
        proc.wait()


if __name__ == "__main__":
    sys.exit(main())
