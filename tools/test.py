#!/usr/bin/env python3
"""Aster OS test entry point.

Runs host tests, a clean build, a headless QEMU boot smoke test, and asserts
that the serial log contains the expected milestones and no faults.

Usage:
  python tools/test.py            # host tests + build + boot smoke
  python tools/test.py --ui       # also run the scripted UI interaction test
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from build import find_cargo  # noqa: E402
from run import Qmp, DATA_IMAGE, drive_args, find_ovmf, find_qemu  # noqa: E402

MARKERS = [
    "BOOT-LOADER-START",
    "BOOT-HANDOFF-OK",
    "KERNEL-ENTRY-OK",
    "KERNEL-READY",
    "DESKTOP-READY",
]
STORAGE_MARKERS = [
    "storage: FAT32",
    "storage: FAT32 read self-test passed",
]
NETWORK_MARKERS = [
    "e1000: 52:54:00:12:34:56",
    "dhcp: bound 10.0.2.15",
]
FORBIDDEN = ["KERNEL PANIC", "user fault", "PANIC:", "system halted"]


def run(command: list[str]) -> None:
    print("$ " + " ".join(command))
    result = subprocess.run(command, cwd=ROOT)
    if result.returncode != 0:
        raise SystemExit(f"command failed: {' '.join(command)}")


def boot_smoke() -> None:
    out = ROOT / "build" / "test"
    out.mkdir(parents=True, exist_ok=True)
    serial = out / "serial.log"
    serial.write_text("")
    port = 4470
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(),
        "-display", "none", "-vnc", "127.0.0.1:9",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    print("$ " + " ".join(cmd))
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=20)
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            text = serial.read_text(errors="replace")
            if "DESKTOP-READY" in text:
                break
            if any(token in text for token in FORBIDDEN):
                break
            time.sleep(0.1)
        time.sleep(2.0)
        screenshot = out / "smoke.ppm"
        qmp.command("screendump", filename=str(screenshot))
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    missing = [marker for marker in MARKERS if marker not in text]
    if DATA_IMAGE.exists():
        missing += [marker for marker in STORAGE_MARKERS if marker not in text]
    missing += [marker for marker in NETWORK_MARKERS if marker not in text]
    faults = [token for token in FORBIDDEN if token in text]
    if missing:
        print(f"FAIL: missing boot markers: {missing}")
    if faults:
        print(f"FAIL: faults reported: {faults}")
    if missing or faults:
        print("--- serial tail ---")
        print("".join(ch for ch in text[-4000:] if ch == "\n" or 32 <= ord(ch) < 127))
        raise SystemExit(1)
    print(f"boot smoke passed; screenshot: {screenshot}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ui", action="store_true", help="run the scripted UI test")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    run([sys.executable, "-m", "unittest", "discover", "-s", "tests", "-v"])
    run([find_cargo(), "test", "-p", "aster-motion", "--quiet"])
    if not args.skip_build:
        run([sys.executable, "tools/build.py"])
    boot_smoke()
    if args.ui:
        run([sys.executable, "tools/ui_test.py"])
        run([sys.executable, "tools/graphics_test.py"])
        run([sys.executable, "tools/gfx_test.py"])
    print("ALL TESTS PASSED")


if __name__ == "__main__":
    main()
