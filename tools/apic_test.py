#!/usr/bin/env python3
"""Local APIC / IOAPIC routing acceptance test."""

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

OUT = ROOT / "build" / "apic"


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4576
    proc = subprocess.Popen(
        [
            find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
            "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
            "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
            "-serial", f"file:{serial}", "-monitor", "none",
            *drive_args(data_path=volume),
            "-display", "none", "-vnc", "127.0.0.1:47",
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
        time.sleep(1.0)
        qmp.command("send-key", keys=[{"type": "qcode", "data": "a"}])
        qmp.command(
            "input-send-event",
            events=[
                {"type": "rel", "data": {"axis": "x", "value": 60}},
                {"type": "rel", "data": {"axis": "y", "value": 20}},
            ],
        )
        time.sleep(3.0)
        text = serial.read_text(errors="replace")
        required = [
            "apic: LAPIC 0xfee00000 id 0",
            "ioapic: irq 0 -> gsi 2 vector 32",
            "ioapic: irq 1 -> gsi 1 vector 33",
            "ioapic: irq 12 -> gsi 12 vector 44",
            "apic: active, PIC masked, EOI -> LAPIC",
            "apic: first IRQ irq 0 gsi 2 vector 32",
            "apic: first IRQ irq 1 gsi 1 vector 33",
            "apic: first IRQ irq 12 gsi 12 vector 44",
        ]
        for marker in required:
            if marker not in text:
                failures.append(f"missing APIC evidence: {marker}")
        if "PANIC" in text:
            failures.append("kernel panicked on the APIC path")
        if "desktop: frame 1" not in text:
            failures.append("desktop never reached its first frame on the APIC path")

        if failures:
            print("FAIL")
            for failure in failures:
                print(f"  {failure}")
            return 1
        print("PASS: LAPIC enabled, IOAPIC routes timer/keyboard/mouse, PIC masked")
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
