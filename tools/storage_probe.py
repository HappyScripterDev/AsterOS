#!/usr/bin/env python3
"""Storage acceptance probe: Settings page, missing disk, corrupt volume."""

from __future__ import annotations

import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import Qmp, find_ovmf, find_qemu, net_args, IMAGE  # noqa: E402

OUT = ROOT / "build" / "storage"
DATA = ROOT / "build" / "data.img"


def move(qmp, dx, dy):
    while dx or dy:
        sx = max(-120, min(120, dx))
        sy = max(-120, min(120, dy))
        qmp.command("input-send-event", events=[
            {"type": "rel", "data": {"axis": "x", "value": sx}},
            {"type": "rel", "data": {"axis": "y", "value": sy}},
        ])
        dx -= sx
        dy -= sy
        time.sleep(0.04)
    time.sleep(0.25)


def click(qmp):
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    time.sleep(0.08)
    qmp.command("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    time.sleep(0.9)


def type_text(qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.05)


def boot(label: str, data_disk: Path | None, port: int, wait: float = 9.0):
    serial = OUT / f"{label}.log"
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        *net_args(),
    ]
    if data_disk is not None:
        cmd += ["-drive", f"file={data_disk},format=raw,if=ide,index=1"]
    cmd += [
        "-display", "none", "-vnc", f"127.0.0.1:{port - 4460}",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    return proc, serial, port


def open_storage(qmp, label: str) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}])
    time.sleep(0.8)
    type_text(qmp, "set")
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])
    time.sleep(1.8)
    move(qmp, 250 - 640, 337 - 400)
    click(qmp)
    qmp.command("screendump", filename=str((OUT / f"{label}.ppm").resolve()))


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    failures = []

    # 1. Normal volume.
    proc, serial, port = boot("with-disk", DATA, 4485)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9.0)
        open_storage(qmp, "01-settings-storage")
        qmp.command("quit")
        qmp.close()
    finally:
        proc.wait(timeout=6)

    # 2. No data disk: the desktop must still start.
    proc, serial, port = boot("no-disk", None, 4486)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9.0)
        qmp.command("screendump", filename=str((OUT / "02-no-disk.ppm").resolve()))
        qmp.command("quit")
        qmp.close()
    finally:
        proc.wait(timeout=6)
    text = serial.read_text(errors="replace")
    if "DESKTOP-READY" not in text:
        failures.append("no-disk boot did not reach the desktop")
    if "storage: no persistent volume mounted" not in text:
        failures.append("no-disk boot did not report the missing volume")

    # 3. Corrupt FAT boot sector: mount must fail, boot must continue.
    corrupt = OUT / "corrupt.img"
    shutil.copyfile(DATA, corrupt)
    with open(corrupt, "r+b") as f:
        f.seek(2048 * 512)
        f.write(b"\0" * 512)
    proc, serial, port = boot("corrupt", corrupt, 4487)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9.0)
        qmp.command("screendump", filename=str((OUT / "03-corrupt.ppm").resolve()))
        qmp.command("quit")
        qmp.close()
    finally:
        proc.wait(timeout=6)
    text = serial.read_text(errors="replace")
    if "DESKTOP-READY" not in text:
        failures.append("corrupt-volume boot did not reach the desktop")
    if "not a usable FAT32 volume" not in text:
        failures.append("corrupt volume was not rejected")
    if "KERNEL PANIC" in text or "user fault" in text:
        failures.append("corrupt volume caused a fault")

    if failures:
        for failure in failures:
            print("FAIL:", failure)
        return 1
    print("PASS: storage Settings probe, missing-disk fallback and corrupt-volume handling")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
