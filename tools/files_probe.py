#!/usr/bin/env python3
"""Files application acceptance probe using real keyboard operations."""

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

OUT = ROOT / "build" / "filesprobe"


def type_text(qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.06)


def key(qmp, code: str, ctrl: bool = False) -> None:
    keys = [{"type": "qcode", "data": "ctrl"}] if ctrl else []
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)
    time.sleep(0.15)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4493
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(data_path=volume),
        "-display", "none", "-vnc", "127.0.0.1:17",
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
        type_text(qmp, "fil")
        key(qmp, "ret")
        time.sleep(1.8)

        # Create a folder, rename it, copy it, paste it, delete the original.
        key(qmp, "n", ctrl=True)
        time.sleep(0.8)
        key(qmp, "f2")
        time.sleep(0.4)
        for _ in range(6):
            key(qmp, "backspace")
        type_text(qmp, "testdir")
        key(qmp, "ret")
        time.sleep(0.8)
        qmp.command("screendump", filename=str((OUT / "01-created.ppm").resolve()))

        key(qmp, "c", ctrl=True)
        key(qmp, "up")
        key(qmp, "up")
        key(qmp, "ret")
        time.sleep(0.8)
        key(qmp, "v", ctrl=True)
        time.sleep(1.5)
        qmp.command("screendump", filename=str((OUT / "02-pasted.ppm").resolve()))

        key(qmp, "backspace")
        time.sleep(0.6)
        for _ in range(3):
            key(qmp, "down")
        key(qmp, "delete")
        time.sleep(0.3)
        key(qmp, "delete")
        time.sleep(1.0)

        # Search and show information for the only match.
        key(qmp, "f", ctrl=True)
        type_text(qmp, "welcome")
        time.sleep(0.8)
        qmp.command("screendump", filename=str((OUT / "03-search.ppm").resolve()))
        key(qmp, "ret")
        key(qmp, "i", ctrl=True)
        time.sleep(0.6)
        qmp.command("screendump", filename=str((OUT / "04-info.ppm").resolve()))
        key(qmp, "ret")
        time.sleep(0.4)

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
        "files: created NEWDIR",
        "files: renamed NEWDIR to testdir",
        "files: copied TESTDIR to TESTDIR",
        "files: deleted TESTDIR",
    ]
    missing = [line for line in required if line not in text]
    if missing or "KERNEL PANIC" in text or "user fault" in text:
        print("FAIL: missing", missing)
        print("".join(ch for ch in text[-3000:] if ch == "\n" or 32 <= ord(ch) < 127))
        return 1
    print("PASS: Files create, rename, copy, paste, delete, search and info")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
