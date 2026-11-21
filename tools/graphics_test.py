#!/usr/bin/env python3
"""Aster OS 0.5.0 graphics, motion and window-management acceptance test.

Drives a real QEMU desktop, checks rendered pixels for real state changes
(minimize/restore, overview dimming, workspace switching), exercises tabs and
the Graphics & Motion settings page, then runs a rapid interaction stress pass.
"""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from fatread import extract_file  # noqa: E402
from image import build_data_image  # noqa: E402
from run import Qmp, drive_args, find_ovmf, find_qemu  # noqa: E402
from seeds import SEED_FILES  # noqa: E402

OUT = ROOT / "build" / "graphics"
POS = [640, 400]


def key(qmp: Qmp, code: str, mods: tuple[str, ...] = ()) -> None:
    keys = [{"type": "qcode", "data": value} for value in mods]
    keys.append({"type": "qcode", "data": code})
    qmp.command("send-key", keys=keys)


def type_text(qmp: Qmp, text: str) -> None:
    special = {" ": "spc", "/": "slash", ".": "dot", "-": "minus"}
    for ch in text:
        key(qmp, special.get(ch, ch))
        time.sleep(0.04)


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
        time.sleep(0.03)
    time.sleep(0.2)


def click(qmp: Qmp) -> None:
    qmp.command(
        "input-send-event",
        events=[{"type": "btn", "data": {"down": True, "button": "left"}}],
    )
    time.sleep(0.07)
    qmp.command(
        "input-send-event",
        events=[{"type": "btn", "data": {"down": False, "button": "left"}}],
    )
    time.sleep(0.6)


def hold_key(qmp: Qmp, code: str, down: bool) -> None:
    qmp.command(
        "input-send-event",
        events=[
            {
                "type": "key",
                "data": {"down": down, "key": {"type": "qcode", "data": code}},
            }
        ],
    )


def open_app(qmp: Qmp, query: str) -> None:
    key(qmp, "spc", ("ctrl",))
    time.sleep(0.65)
    type_text(qmp, query)
    key(qmp, "ret")
    time.sleep(1.3)


def shot(qmp: Qmp, name: str) -> Path:
    path = OUT / name
    qmp.command("screendump", filename=str(path.resolve()))
    return path


def pixels(path: Path):
    from PIL import Image

    image = Image.open(path).convert("RGB")
    return image


def pixel(image, x: int, y: int) -> tuple[int, int, int]:
    return image.getpixel((x, y))


def bright(rgb: tuple[int, int, int]) -> float:
    return (rgb[0] + rgb[1] + rgb[2]) / 3.0


def near(a, b, tolerance: int = 14) -> bool:
    return all(abs(a[i] - b[i]) <= tolerance for i in range(3))


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4560
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(data_path=volume),
        "-display", "none", "-vnc", "127.0.0.1:24",
        "-qmp", f"tcp:127.0.0.1:{port},server=on,wait=off",
    ]
    proc = subprocess.Popen(cmd, cwd=ROOT)
    failures: list[str] = []
    try:
        qmp = Qmp(port, timeout=15)
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            if "DESKTOP-READY" in serial.read_text(errors="replace"):
                break
            time.sleep(0.1)
        time.sleep(1.2)
        baseline = pixels(shot(qmp, "00-baseline.ppm"))

        open_app(qmp, "term")
        type_text(qmp, "help")
        key(qmp, "ret")
        time.sleep(0.8)
        open_app(qmp, "files")
        windows = pixels(shot(qmp, "01-windows.ppm"))

        # Minimize the focused Files window and verify the old area is repainted.
        key(qmp, "m", ("ctrl",))
        time.sleep(1.4)
        minimized = pixels(shot(qmp, "02-minimized.ppm"))
        if not near(pixel(minimized, 980, 540), pixel(baseline, 980, 540), 20):
            failures.append("minimize left residue in the old window area")

        # Restore from the Files dock icon.
        move_to(qmp, (422, 780))
        click(qmp)
        time.sleep(1.2)
        restored = pixels(shot(qmp, "03-restored.ppm"))
        if bright(pixel(restored, 700, 450)) < 190:
            failures.append("dock click did not restore the Files window")

        # Window overview.
        key(qmp, "f3")
        time.sleep(1.0)
        overview = pixels(shot(qmp, "04-overview.ppm"))
        if bright(pixel(overview, 600, 10)) > bright(pixel(baseline, 600, 10)) - 45:
            failures.append("overview did not dim the desktop")
        key(qmp, "esc")
        time.sleep(1.3)

        # Held-Alt quick switcher.
        hold_key(qmp, "alt", True)
        time.sleep(0.12)
        hold_key(qmp, "tab", True)
        time.sleep(0.12)
        hold_key(qmp, "tab", False)
        time.sleep(0.5)
        switcher = pixels(shot(qmp, "05-switcher.ppm"))
        if bright(pixel(switcher, 640, 400)) < 180:
            failures.append("quick switcher panel is not visible")
        if bright(pixel(switcher, 600, 10)) > bright(pixel(baseline, 600, 10)) - 30:
            failures.append("quick switcher did not dim the desktop")
        hold_key(qmp, "alt", False)
        time.sleep(0.8)

        # Workspace slide to desktop 2 and back.
        key(qmp, "right", ("ctrl",))
        time.sleep(1.0)
        desktop2 = pixels(shot(qmp, "06-desktop2.ppm"))
        if not near(pixel(desktop2, 500, 300), pixel(baseline, 500, 300), 22):
            failures.append("desktop 2 still shows desktop 1 window content")
        key(qmp, "left", ("ctrl",))
        time.sleep(1.0)
        desktop1 = pixels(shot(qmp, "07-desktop1.ppm"))
        if bright(pixel(desktop1, 700, 450)) < 190:
            failures.append("returning to desktop 1 lost the window")

        # Text editor tabs.
        open_app(qmp, "text")
        type_text(qmp, "one")
        key(qmp, "t", ("ctrl",))
        time.sleep(0.5)
        type_text(qmp, "two")
        time.sleep(0.6)
        shot(qmp, "08-tabs.ppm")
        move_to(qmp, (350, 108))
        click(qmp)
        time.sleep(0.6)
        key(qmp, "pgdn", ("ctrl",))
        time.sleep(0.5)

        # Graphics & Motion settings page.
        open_app(qmp, "settings")
        for _ in range(9):
            key(qmp, "down")
            time.sleep(0.06)
        time.sleep(0.8)
        shot(qmp, "09-graphics-settings.ppm")

        # Toggle Window animations off and verify the setting reached storage.
        move_to(qmp, (738, 510))
        click(qmp)
        time.sleep(1.2)
        shot(qmp, "09b-window-animations-off.ppm")
        stored = extract_file(str(volume), "/SETTINGS.CNF") or b""
        if b"motion=0,1,1,1,1" not in stored:
            failures.append(f"graphics setting was not persisted: {stored!r}")

        # Rapid interaction stress: close everything, open overview, switch
        # desktops, then prove the shell still accepts input.
        for _ in range(3):
            key(qmp, "w", ("ctrl",))
            time.sleep(0.25)
        key(qmp, "right", ("ctrl",))
        time.sleep(0.4)
        key(qmp, "f3")
        time.sleep(0.5)
        key(qmp, "esc")
        time.sleep(0.5)
        open_app(qmp, "term")
        type_text(qmp, "uname")
        key(qmp, "ret")
        time.sleep(0.8)
        shot(qmp, "10-after-stress.ppm")
        qmp.command("quit")
        qmp.close()
    finally:
        try:
            proc.wait(timeout=8)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    text = serial.read_text(errors="replace")
    required = [
        "DESKTOP-READY",
        "text: tab 2 of 2",
        "text: tab 1 of 2",
        "settings: section 9",
    ]
    for marker in required:
        if marker not in text:
            failures.append(f"missing evidence: {marker}")
    for token in ("KERNEL PANIC", "desktop: PANIC", "PANIC:"):
        if token in text:
            failures.append(f"fatal error: {token}")
    if "uname" in text and "Aster OS" not in text:
        failures.append("terminal did not answer after the stress pass")

    if failures:
        print("FAIL")
        for failure in failures:
            print("  " + failure)
        print("--- serial tail ---")
        print("".join(ch for ch in text[-4000:] if ch == "\n" or 32 <= ord(ch) < 127))
        return 1
    print("PASS: dirty compositor, window animations, overview, switcher, workspaces and tabs")
    print(f"screenshots: {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
