#!/usr/bin/env python3
"""Engine performance smoke test.

Boots the desktop, opens two apps, waits for real frames and reads the
compositor's own frame log. It asserts that the engine presented content, that
draw/present stay inside generous software-renderer budgets, and that the shell
did not panic. This is a smoke test, not a benchmark: QEMU TCG wall time is not
representative of hardware.
"""

from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from image import build_data_image  # noqa: E402
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "perf"
FRAME = re.compile(
    r"desktop: frame (\d+) .*? input (\d+) draw (\d+) present (\d+) "
    r"dirty (\d+) px, full (\d+), skipped (\d+)"
)


def key(qmp: Qmp, code: str, mods: tuple[str, ...] = ()) -> None:
    keys = [{"type": "qcode", "data": value} for value in mods]
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)


def type_text(qmp: Qmp, text: str) -> None:
    for ch in text:
        key(qmp, {" ": "spc"}.get(ch, ch))
        time.sleep(0.04)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4566
    proc = subprocess.Popen(
        [
            find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
            "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
            "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
            "-serial", f"file:{serial}", "-monitor", "none",
            *drive_args(data_path=volume),
            "-display", "none", "-vnc", "127.0.0.1:26",
            "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
        ],
        cwd=ROOT,
    )
    try:
        qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        time.sleep(1.2)
        key(qmp, "spc", ("ctrl",))
        time.sleep(0.7)
        type_text(qmp, "term")
        key(qmp, "ret")
        time.sleep(3.0)
        type_text(qmp, "help")
        key(qmp, "ret")
        time.sleep(3.0)
        key(qmp, "spc", ("ctrl",))
        time.sleep(0.7)
        type_text(qmp, "files")
        key(qmp, "ret")
        time.sleep(18.0)
        qmp.command("quit")
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    if "PANIC" in text:
        print("FAIL")
        print("  desktop panicked during the performance run")
        return 1

    frames = [tuple(int(value) for value in match.groups()) for match in FRAME.finditer(text)]
    if not frames:
        print("FAIL")
        print("  no compositor frame statistics were logged")
        return 1

    worst_draw = max(frame[2] for frame in frames)
    worst_present = max(frame[3] for frame in frames)
    painted = max(frame[4] for frame in frames)
    failures: list[str] = []
    if painted == 0:
        failures.append("no frame reported dirty pixels")
    if worst_draw > 150:
        failures.append(f"draw time {worst_draw} ms exceeds the 150 ms software budget")
    if worst_present > 80:
        failures.append(f"present time {worst_present} ms exceeds the 80 ms software budget")

    if failures:
        print("FAIL")
        for failure in failures:
            print(f"  {failure}")
        return 1
    print(
        "PASS: compositor performance smoke "
        f"({len(frames)} logged frames, worst draw {worst_draw} ms, "
        f"worst present {worst_present} ms, max dirty {painted} px)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
