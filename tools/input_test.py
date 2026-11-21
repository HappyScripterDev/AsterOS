#!/usr/bin/env python3
"""PS/2 IntelliMouse wheel and Files scrolling acceptance test."""

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

OUT = ROOT / "build" / "input"
POS = [640, 400]


def key(qmp: Qmp, code: str, mods: tuple[str, ...] = ()) -> None:
    keys = [{"type": "qcode", "data": value} for value in mods]
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)


def type_text(qmp: Qmp, text: str) -> None:
    for ch in text:
        key(qmp, ch)
        time.sleep(0.08)


def move_rel(qmp: Qmp, dx: int, dy: int) -> None:
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


def move_to(qmp: Qmp, target: tuple[int, int]) -> None:
    while POS[0] != target[0] or POS[1] != target[1]:
        dx = max(-120, min(120, target[0] - POS[0]))
        dy = max(-120, min(120, target[1] - POS[1]))
        move_rel(qmp, dx, dy)
    time.sleep(0.2)


def wheel(qmp: Qmp, down: bool) -> None:
    button = "wheel-down" if down else "wheel-up"
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": button}}])
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": button}}])
    time.sleep(0.4)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    files = dict(SEED_FILES)
    for index in range(24):
        files[f"/F{index:02}.TXT"] = b"wheel test file\n"
    build_data_image(str(volume), files)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4578
    proc = subprocess.Popen(
        [
            find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
            "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
            "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
            "-serial", f"file:{serial}", "-monitor", "none",
            *drive_args(data_path=volume),
            "-display", "none", "-vnc", "127.0.0.1:48",
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
        type_text(qmp, "files")
        key(qmp, "ret")
        time.sleep(7.0)
        move_to(qmp, (600, 300))
        # Move the selection down so the list has somewhere to scroll.
        for _ in range(12):
            key(qmp, "down")
            time.sleep(0.1)
        time.sleep(1.0)
        for _ in range(3):
            wheel(qmp, True)
        time.sleep(2.0)
        text = serial.read_text(errors="replace")
        required = [
            "ps2: IntelliMouse wheel enabled",
            "ps2: first wheel event",
            "files: wheel scroll",
        ]
        for marker in required:
            if marker not in text:
                failures.append(f"missing wheel evidence: {marker}")
        if "files: wheel scroll" not in text:
            failures.append("Files did not apply a wheel scroll")
        if "PANIC" in text:
            failures.append("panic during wheel input")

        if failures:
            print("FAIL")
            for failure in failures:
                print(f"  {failure}")
            return 1
        print("PASS: IntelliMouse wheel detected and Files scrolled")
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
