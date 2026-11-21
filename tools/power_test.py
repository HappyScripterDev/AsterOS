#!/usr/bin/env python3
"""Shutdown/restart acceptance from the Terminal and the desktop menu."""

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

OUT = ROOT / "build" / "power"
POS = [640, 400]


def key(qmp: Qmp, code: str, mods: tuple[str, ...] = ()) -> None:
    keys = [{"type": "qcode", "data": value} for value in mods]
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)


def type_text(qmp: Qmp, text: str) -> None:
    for ch in text:
        key(qmp, {" ": "spc"}.get(ch, ch))
        time.sleep(0.12)


def move_to(qmp: Qmp, target: tuple[int, int]) -> None:
    while POS[0] != target[0] or POS[1] != target[1]:
        dx = max(-120, min(120, target[0] - POS[0]))
        dy = max(-120, min(120, target[1] - POS[1]))
        qmp.command(
            "input-send-event",
            events=[
                {"type": "rel", "data": {"axis": "x", "value": dx}},
                {"type": "rel", "data": {"axis": "y", "value": dy}},
            ],
        )
        POS[0] += dx
        POS[1] += dy
        time.sleep(0.04)
    time.sleep(0.2)


def click(qmp: Qmp, may_disconnect: bool = False) -> None:
    try:
        qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
        time.sleep(0.1)
        qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    except (ConnectionResetError, ConnectionAbortedError, OSError):
        if not may_disconnect:
            raise
    time.sleep(0.8)


class Boot:
    def __init__(self, name: str, port: int):
        self.dir = OUT / name
        self.dir.mkdir(parents=True, exist_ok=True)
        volume = self.dir / "data.img"
        build_data_image(str(volume), SEED_FILES)
        self.serial = self.dir / "serial.log"
        self.serial.write_text("")
        self.proc = subprocess.Popen(
            [
                find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
                "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
                "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
                "-serial", f"file:{self.serial}", "-monitor", "none",
                *drive_args(data_path=volume),
                "-display", "none", "-vnc", f"127.0.0.1:{port + 20}",
                "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
            ],
            cwd=ROOT,
        )
        self.qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in self.text():
                break
            time.sleep(0.1)
        time.sleep(1.2)

    def text(self) -> str:
        return self.serial.read_text(errors="replace")

    def wait_log(self, marker: str, seconds: float = 15.0) -> bool:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if marker in self.text():
                return True
            time.sleep(0.1)
        return False

    def wait_exit(self, seconds: float = 12.0) -> bool:
        try:
            self.proc.wait(timeout=seconds)
            return True
        except subprocess.TimeoutExpired:
            return False

    def close(self) -> None:
        try:
            self.qmp.close()
        except Exception:
            pass
        if self.proc.poll() is None:
            self.proc.kill()
        self.proc.wait()


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    failures: list[str] = []

    # Terminal restart.
    boot = Boot("terminal", 4572)
    try:
        key(boot.qmp, "spc", ("ctrl",))
        time.sleep(2.5)
        type_text(boot.qmp, "term")
        key(boot.qmp, "ret")
        time.sleep(6.0)
        type_text(boot.qmp, "reboot")
        time.sleep(1.0)
        key(boot.qmp, "ret")
        if not boot.wait_log("acpi: reset requested"):
            failures.append("terminal reboot did not reach the ACPI reset path")
        if not boot.wait_exit():
            failures.append("QEMU did not exit on terminal reboot (-no-reboot)")
        if "PANIC" in boot.text():
            failures.append("panic during terminal reboot")
    finally:
        boot.close()

    # Desktop menu restart with confirmation.
    boot = Boot("desktop", 4574)
    try:
        move_to(boot.qmp, (60, 16))
        click(boot.qmp)
        time.sleep(2.0)
        move_to(boot.qmp, (100, 110))
        click(boot.qmp)
        time.sleep(2.0)
        move_to(boot.qmp, (741, 458))
        click(boot.qmp, may_disconnect=True)
        if not boot.wait_log("acpi: reset requested"):
            failures.append("desktop restart confirmation did not reach the ACPI reset path")
        if not boot.wait_exit():
            failures.append("QEMU did not exit on desktop restart")
        if "PANIC" in boot.text():
            failures.append("panic during desktop restart")
    finally:
        boot.close()

    if failures:
        print("FAIL")
        for failure in failures:
            print(f"  {failure}")
        return 1
    print("PASS: Terminal and desktop restart through ACPI")
    return 0


if __name__ == "__main__":
    sys.exit(main())
