#!/usr/bin/env python3
"""Aster OS graphics-engine acceptance probe.

Covers the 0.5.0 extension behaviours that the older graphics test does not:
per-window surface composition after a move, fullscreen chrome hiding and
restore, and direct jumps through the top-bar pager.
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

OUT = ROOT / "build" / "gfx"
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
    time.sleep(0.05)


def move_to(qmp: Qmp, target: tuple[int, int]) -> None:
    while POS[0] != target[0] or POS[1] != target[1]:
        dx = max(-120, min(120, target[0] - POS[0]))
        dy = max(-120, min(120, target[1] - POS[1]))
        move_rel(qmp, dx, dy)
    time.sleep(0.2)


def button(qmp: Qmp, down: bool) -> None:
    qmp.command(
        "input-send-event",
        events=[{"type": "btn", "data": {"down": down, "button": "left"}}],
    )
    time.sleep(0.08)


def click(qmp: Qmp) -> None:
    button(qmp, True)
    button(qmp, False)
    time.sleep(0.6)


def shot(qmp: Qmp, name: str) -> Path:
    path = OUT / name
    qmp.command("screendump", filename=str(path.resolve()))
    return path


def pixels(path: Path):
    from PIL import Image

    return Image.open(path).convert("RGB")


def pixel(image, x: int, y: int) -> tuple[int, int, int]:
    return image.getpixel((x, y))


def bright(rgb: tuple[int, int, int]) -> float:
    return (rgb[0] + rgb[1] + rgb[2]) / 3.0


def near(a, b, tolerance: int = 18) -> bool:
    return all(abs(a[i] - b[i]) <= tolerance for i in range(3))


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    volume = OUT / "data.img"
    build_data_image(str(volume), SEED_FILES)
    serial = OUT / "serial.log"
    serial.write_text("")
    port = 4562
    cmd = [
        find_qemu(), "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M",
        "-drive", f"if=pflash,format=raw,readonly=on,file={find_ovmf()}",
        "-device", "VGA", "-rtc", "base=localtime", "-no-reboot",
        "-serial", f"file:{serial}", "-monitor", "none",
        *drive_args(data_path=volume),
        "-display", "none", "-vnc", "127.0.0.1:25",
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

        # Open a terminal so the compositor has real cached content.
        key(qmp, "spc", ("ctrl",))
        time.sleep(0.7)
        type_text(qmp, "term")
        key(qmp, "ret")
        time.sleep(2.0)

        # Drag the titlebar right. The old area must be restored and the new
        # titlebar must be composed from the cached window surface.
        move_to(qmp, (400, 75))
        button(qmp, True)
        for _ in range(3):
            move_rel(qmp, 60, 0)
            time.sleep(0.15)
        button(qmp, False)
        time.sleep(4.0)
        moved = pixels(shot(qmp, "01-moved.ppm"))
        if not near(pixel(moved, 300, 75), pixel(baseline, 300, 75), 30):
            failures.append("window move left content residue at the old titlebar")
        if bright(pixel(moved, 520, 75)) < 200:
            failures.append("window titlebar is missing after the move")

        # Fullscreen hides the dock; Escape restores it.
        key(qmp, "f11")
        time.sleep(6.0)
        fullscreen = pixels(shot(qmp, "02-fullscreen.ppm"))
        if near(pixel(fullscreen, 948, 760), pixel(baseline, 948, 760), 12):
            failures.append("fullscreen left the dock visible")
        key(qmp, "esc")
        time.sleep(6.0)
        restored = pixels(shot(qmp, "03-restored.ppm"))
        if not near(pixel(restored, 948, 760), pixel(baseline, 948, 760), 24):
            failures.append("Escape did not restore the dock")

        # Jump to desktop 2 through the top-bar pager and back.
        move_to(qmp, (633, 16))
        click(qmp)
        time.sleep(6.0)
        desktop2 = pixels(shot(qmp, "04-desktop2.ppm"))
        if not near(pixel(desktop2, 520, 200), pixel(baseline, 520, 200), 30):
            failures.append("pager jump left window content on desktop 2")
        if bright(pixel(desktop2, 600, 10)) < 200:
            failures.append("pager jump removed the top bar")
        move_to(qmp, (619, 16))
        click(qmp)
        time.sleep(6.0)
        back = pixels(shot(qmp, "05-desktop1.ppm"))
        if bright(pixel(back, 520, 75)) < 190:
            failures.append("pager did not return to the window on desktop 1")

        # Hold a dragged window against the right edge: it should move to the
        # next desktop and the pager should show the second mark active.
        move_to(qmp, (520, 75))
        button(qmp, True)
        for _ in range(6):
            move_rel(qmp, 110, 0)
            time.sleep(0.1)
        move_to(qmp, (1272, 76))
        time.sleep(12.0)
        button(qmp, False)
        time.sleep(8.0)
        dropped = pixels(shot(qmp, "06-drag-desktop.ppm"))
        blue = [
            x
            for x in range(560, 720)
            if pixel(dropped, x, 12)[2] - pixel(dropped, x, 12)[0] > 40
        ]
        if not blue or not (624 <= min(blue) and max(blue) <= 642):
            failures.append("edge dwell did not move the window to desktop 2")
        if bright(pixel(dropped, 1200, 300)) < 220:
            failures.append("dropped window is not visible on desktop 2")

        if "PANIC" in serial.read_text(errors="replace"):
            failures.append("desktop panicked during the graphics probe")

        if failures:
            print("FAIL")
            for failure in failures:
                print(f"  {failure}")
            return 1
        print("PASS: window surfaces, fullscreen and pager")
        return 0
    finally:
        try:
            qmp.command("quit")
        except Exception:
            pass
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()


if __name__ == "__main__":
    sys.exit(main())
