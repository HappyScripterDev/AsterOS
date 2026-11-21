#!/usr/bin/env python3
"""ACPI fixed-table and S5 shutdown acceptance test.

Boots QEMU q35, verifies the parsed RSDP/XSDT/FADT/MADT/MCFG/HPET values from
the kernel log, then runs `shutdown` in the Terminal and checks that the guest
logged S5 and the QEMU process exited without a fault.
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

OUT = ROOT / "build" / "acpi"


def key(qmp: Qmp, code: str, mods: tuple[str, ...] = ()) -> None:
    keys = [{"type": "qcode", "data": value} for value in mods]
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)


def type_text(qmp: Qmp, text: str) -> None:
    for ch in text:
        key(qmp, {" ": "spc"}.get(ch, ch))
        time.sleep(0.12)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4568
    proc = subprocess.Popen(
        [
            find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
            "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
            "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
            "-serial", f"file:{serial}", "-monitor", "none",
            *drive_args(data_path=volume),
            "-display", "none", "-vnc", "127.0.0.1:27",
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
        text = serial.read_text(errors="replace")
        required = [
            "acpi: RSDP rev 2",
            "acpi: FADT",
            "acpi: MADT",
            "lapic=0xfee00000",
            "cpus=1",
            "ioapics=1",
            "acpi: IOAPIC id 0 at 0xfec00000",
            "acpi: MCFG",
            "base 0xe0000000",
            "acpi: HPET",
            "base 0xfed00000",
        ]
        for marker in required:
            if marker not in text:
                failures.append(f"missing ACPI evidence: {marker}")

        time.sleep(1.0)
        key(qmp, "spc", ("ctrl",))
        time.sleep(2.5)
        type_text(qmp, "term")
        key(qmp, "ret")
        time.sleep(6.0)
        type_text(qmp, "shutdown")
        time.sleep(1.0)
        key(qmp, "ret")
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if "acpi: S5 shutdown" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        text = serial.read_text(errors="replace")
        if "acpi: S5 shutdown" not in text:
            failures.append("shutdown did not reach the ACPI S5 path")
        if "PANIC" in text:
            failures.append("kernel/desktop panicked during shutdown")
        try:
            proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            failures.append("QEMU did not power off after S5")

        if failures:
            print("FAIL")
            for failure in failures:
                print(f"  {failure}")
            return 1
        print("PASS: ACPI tables parsed and S5 shutdown powered off QEMU")
        return 0
    finally:
        if qmp is not None:
            try:
                qmp.close()
            except Exception:
                pass
        if proc.poll() is None:
            proc.kill()
        proc.wait()


if __name__ == "__main__":
    sys.exit(main())
