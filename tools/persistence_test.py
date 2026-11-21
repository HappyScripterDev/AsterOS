#!/usr/bin/env python3
"""Persistence acceptance test: write a file, reboot, verify it survives.

Uses a copy of build/data.img so the interactive volume is never modified.
The host FAT32 verifier checks the bytes written by the guest; the second boot
reads the same file back on screen.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from image import build_data_image, verify_image  # noqa: E402
from run import Qmp, find_ovmf, find_qemu, net_args, IMAGE  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "persist"
DATA = OUT / "data.img"
SEEDS = dict(SEED_FILES)
PAYLOAD = "hello-from-first-boot"
SETTINGS_AFTER = (
    b"version=1\naccent=green\nanimations=1.00\n"
    b"wallpaper=0\nreduce_motion=0\nbrightness=1.00\n"
    b"gfx=1,1,1,1,60,1.00\nmotion=1,1,1,1,1\ndock=0,1.00,0.45,0,1\n"
)


def type_text(qmp: Qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        qmp.command("send-key", keys=[{"type": "qcode", "data": special.get(ch, ch)}])
        time.sleep(0.05)


def enter(qmp: Qmp) -> None:
    qmp.command("send-key", keys=[{"type": "qcode", "data": "ret"}])


def open_terminal(qmp: Qmp) -> None:
    qmp.command(
        "send-key",
        keys=[{"type": "qcode", "data": "ctrl"}, {"type": "qcode", "data": "spc"}],
    )
    time.sleep(0.8)
    type_text(qmp, "term")
    enter(qmp)
    time.sleep(1.6)


def run_boot(script, label: str) -> None:
    port = 4483
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{OUT / (label + '.log')}", "-monitor", "none",
        "-drive", f"file={IMAGE},format=raw,if=ide,index=0",
        *net_args(),
        "-drive", f"file={DATA},format=raw,if=ide,index=1",
        "-display", "none", "-vnc", f"127.0.0.1:{14 + len(label)}",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    try:
        qmp = Qmp(port, timeout=15)
        time.sleep(9.0)
        script(qmp)
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


def first_boot(qmp: Qmp) -> None:
    open_terminal(qmp)
    type_text(qmp, f"write /home/user/persist.txt {PAYLOAD}")
    enter(qmp)
    time.sleep(1.0)
    type_text(qmp, "accent green")
    enter(qmp)
    time.sleep(1.0)
    qmp.command("screendump", filename=str((OUT / "01-write.ppm").resolve()))


def second_boot(qmp: Qmp) -> None:
    open_terminal(qmp)
    type_text(qmp, "cat /home/user/persist.txt")
    enter(qmp)
    time.sleep(0.8)
    type_text(qmp, "cat /home/user/settings.cnf")
    enter(qmp)
    time.sleep(0.8)
    type_text(qmp, "ls /home/user")
    enter(qmp)
    time.sleep(1.0)
    qmp.command("screendump", filename=str((OUT / "02-read.ppm").resolve()))


def check_volume(stage: str, settings: bytes) -> bool:
    expected = dict(SEEDS)
    expected["/SETTINGS.CNF"] = settings
    expected["/SETTINGS.BAK"] = (
        b"version=1\naccent=blue\nanimations=1.00\n"
        b"wallpaper=0\nreduce_motion=0\nbrightness=1.00\n"
    )
    expected["/PERSIST.TXT"] = PAYLOAD.encode()
    problems = verify_image(str(DATA), expected)
    bad = [line for line in problems if not line.startswith("ok ")]
    if bad:
        print(f"FAIL ({stage}):")
        for line in bad:
            print("  " + line)
        return False
    print(f"PASS ({stage}): host verifier read {len(expected)} files, including /PERSIST.TXT")
    return True


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    if not (ROOT / "build" / "data.img").exists():
        print("error: build/data.img missing; run python tools/build.py --fresh-data")
        return 2
    build_data_image(str(DATA), SEEDS)
    for stale in ("01-write.ppm", "02-read.ppm"):
        (OUT / stale).unlink(missing_ok=True)

    run_boot(first_boot, "first")
    if not check_volume("after first boot", SETTINGS_AFTER):
        return 1

    run_boot(second_boot, "second")
    if not check_volume("after reboot", SETTINGS_AFTER):
        return 1

    text = (OUT / "second.log").read_text(errors="replace")
    if "user fault" in text or "PANIC" in text:
        print("FAIL: second boot reported a fault")
        return 1
    print("PASS: second boot started the desktop with the same volume")
    print(f"screenshots: {OUT / '01-write.ppm'} and {OUT / '02-read.ppm'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
