#!/usr/bin/env python3
"""Settings reliability tests: corruption, validation and valid persistence."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from fatread import extract_file as read_file  # noqa: E402
from image import build_data_image  # noqa: E402
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "settingsprobe"
DEFAULT = (
    b"version=1\naccent=blue\nanimations=1.00\n"
    b"wallpaper=0\nreduce_motion=0\nbrightness=1.00\n"
)


def boot(label: str, volume: Path, port: int) -> str:
    serial = OUT / f"{label}.log"
    serial.write_text("")
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(data_path=volume),
        "-display", "none", "-vnc", f"127.0.0.1:{port - 4460}",
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
        time.sleep(2.0)
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    return serial.read_text(errors="replace")


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    failures: list[str] = []

    # 1. Structurally corrupt file: preserved as BAD, defaults written.
    corrupt = dict(SEED_FILES)
    corrupt["/SETTINGS.CNF"] = b"this is not a settings file\n"
    corrupt_path = OUT / "corrupt.img"
    build_data_image(str(corrupt_path), corrupt)
    text = boot("corrupt", corrupt_path, 4496)
    if "configd: corrupt settings preserved" not in text:
        failures.append("corrupt settings were not reported")
    if read_file(str(corrupt_path), "/SETTINGS.CNF") != DEFAULT:
        failures.append("corrupt settings did not recover to defaults")
    if read_file(str(corrupt_path), "/SETTINGS.BAD") != b"this is not a settings file\n":
        failures.append("corrupt settings were not preserved as SETTINGS.BAD")

    # 2. Structurally valid but semantically invalid values are corrected.
    invalid = dict(SEED_FILES)
    invalid["/SETTINGS.CNF"] = (
        b"version=1\naccent=notacolour\nanimations=99\n"
        b"wallpaper=99\nreduce_motion=7\nbrightness=9\n"
    )
    invalid_path = OUT / "invalid.img"
    build_data_image(str(invalid_path), invalid)
    boot("invalid", invalid_path, 4497)
    stored = read_file(str(invalid_path), "/SETTINGS.CNF")
    if stored != DEFAULT:
        failures.append(f"invalid values were not normalised: {stored!r}")

    # 3. A valid custom file is served unchanged.
    valid = dict(SEED_FILES)
    valid["/SETTINGS.CNF"] = (
        b"version=1\naccent=green\nanimations=0.75\n"
        b"wallpaper=2\nreduce_motion=1\nbrightness=0.90\n"
    )
    valid_path = OUT / "valid.img"
    build_data_image(str(valid_path), valid)
    boot("valid", valid_path, 4498)
    stored = read_file(str(valid_path), "/SETTINGS.CNF")
    if stored != valid["/SETTINGS.CNF"]:
        failures.append(f"valid settings were changed: {stored!r}")

    if failures:
        for failure in failures:
            print("FAIL:", failure)
        return 1
    print("PASS: settings corruption, validation and valid round-trip")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
