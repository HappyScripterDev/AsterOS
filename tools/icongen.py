#!/usr/bin/env python3
"""Generate Aster's app icon set as original raster artwork.

Each icon is drawn at 128x128 with a squircle body, gradient, inner highlight,
soft shadow and app-specific artwork. The desktop scales these textures down
with bilinear filtering, so small dock icons stay crisp.

Outputs:
  assets/branding/app_icons.bin  9 * 128 * 128 RGBA (0xAARRGGBB little endian)
  user/asterdesk/src/icons_data.rs
"""

from __future__ import annotations

import math
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
FONT_DIR = ROOT / "assets" / "fonts"
OUT = ROOT / "assets" / "branding"
RUST = ROOT / "user" / "asterdesk" / "src" / "icons_data.rs"

SIZE = 128
ICON_NAMES = [
    "terminal", "files", "settings", "calculator", "monitor",
    "text", "notes", "calendar", "photos",
]


def font(size: int, weight: int = 600, mono: bool = False) -> ImageFont.FreeTypeFont:
    path = FONT_DIR / ("JetBrainsMono-Variable.ttf" if mono else "Inter-Variable.ttf")
    f = ImageFont.truetype(str(path), size)
    axes = f.get_variation_axes()
    if axes:
        values = [axis.get("default", axis.get("minimum", 0)) for axis in axes]
        for index, axis in enumerate(axes):
            name = axis.get("name", b"")
            if isinstance(name, bytes):
                name = name.decode(errors="replace")
            if "weight" in name.lower() or "wght" in name.lower():
                values[index] = weight
        f.set_variation_by_axes(values)
    return f


def gradient(size: int, top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    image = Image.new("RGBA", (size, size))
    px = image.load()
    for y in range(size):
        t = y / (size - 1)
        r = round(top[0] + (bottom[0] - top[0]) * t)
        g = round(top[1] + (bottom[1] - top[1]) * t)
        b = round(top[2] + (bottom[2] - top[2]) * t)
        for x in range(size):
            px[x, y] = (r, g, b, 255)
    return image


def squircle(size: int, inset: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        (inset, inset, size - 1 - inset, size - 1 - inset),
        radius=radius,
        fill=255,
    )
    return mask


def body(top: tuple[int, int, int], bottom: tuple[int, int, int], border: int = 0) -> Image.Image:
    base = gradient(SIZE, top, bottom)
    mask = squircle(SIZE, 8, 30)
    out = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    out.paste(base, (0, 0), mask)
    # Soft shadow under the body.
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    shadow.putalpha(mask.point(lambda a: int(a * 0.28)))
    shadow = shadow.filter(ImageFilter.GaussianBlur(5))
    canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    canvas.alpha_composite(shadow, (0, 3))
    canvas.alpha_composite(out)
    # Inner top highlight.
    gloss = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 0))
    gp = gloss.load()
    for y in range(8, 64):
        t = max(0.0, 1.0 - (y - 8) / 48.0)
        for x in range(8, SIZE - 8):
            gp[x, y] = (255, 255, 255, int(26 * t))
    gloss.putalpha(Image.composite(gloss.getchannel("A"), Image.new("L", (SIZE, SIZE), 0), mask))
    canvas.alpha_composite(gloss)
    return canvas


def terminal_icon() -> Image.Image:
    canvas = body((58, 58, 62), (28, 28, 30))
    d = ImageDraw.Draw(canvas)
    white = (245, 245, 247, 255)
    d.line([(34, 50), (52, 64), (34, 78)], fill=white, width=6, joint="curve")
    d.rounded_rectangle((58, 72, 92, 78), radius=3, fill=white)
    d.ellipse((34, 92, 44, 102), fill=(255, 95, 87, 255))
    d.ellipse((48, 92, 58, 102), fill=(254, 188, 46, 255))
    d.ellipse((62, 92, 72, 102), fill=(40, 200, 64, 255))
    return canvas


def files_icon() -> Image.Image:
    canvas = body((109, 200, 255), (10, 132, 255))
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle((28, 38, 72, 54), radius=6, fill=(255, 255, 255, 235))
    d.rounded_rectangle((26, 48, 102, 92), radius=10, fill=(255, 255, 255, 245))
    d.rounded_rectangle((26, 62, 102, 92), radius=10, fill=(236, 246, 255, 255))
    d.line([(34, 70), (94, 70)], fill=(188, 224, 255, 255), width=2)
    return canvas


def gear_polygon(cx: int, cy: int, teeth: int, outer: float, inner: float) -> list[tuple[float, float]]:
    points = []
    for i in range(teeth * 4):
        angle = (i / (teeth * 4)) * 2 * math.pi - math.pi / 2
        radius = outer if (i % 4) in (1, 2) else inner
        points.append((cx + math.cos(angle) * radius, cy + math.sin(angle) * radius))
    return points


def settings_icon() -> Image.Image:
    canvas = body((238, 238, 242), (196, 196, 204))
    d = ImageDraw.Draw(canvas)
    gear = gear_polygon(64, 64, 8, 38, 29)
    d.polygon(gear, fill=(92, 92, 100, 255))
    d.ellipse((46, 46, 82, 82), fill=(238, 238, 242, 255))
    d.ellipse((56, 56, 72, 72), fill=(92, 92, 100, 255))
    return canvas


def calculator_icon() -> Image.Image:
    canvas = body((255, 189, 74), (255, 149, 0))
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle((28, 28, 100, 52), radius=6, fill=(255, 255, 255, 235))
    d.rounded_rectangle((28, 40, 100, 52), radius=6, fill=(255, 250, 240, 255))
    for row in range(3):
        for col in range(3):
            x = 30 + col * 24
            y = 64 + row * 19
            d.rounded_rectangle((x, y, x + 18, y + 13), radius=4, fill=(255, 255, 255, 245))
    return canvas


def monitor_icon() -> Image.Image:
    canvas = body((73, 98, 146), (32, 44, 72))
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle((24, 30, 104, 84), radius=9, fill=(255, 255, 255, 245))
    points = [(32, 70), (44, 60), (56, 66), (68, 48), (80, 58), (96, 42)]
    d.line(points, fill=(40, 200, 100, 255), width=4, joint="curve")
    d.rounded_rectangle((56, 86, 72, 94), radius=2, fill=(240, 244, 252, 255))
    return canvas


def text_icon() -> Image.Image:
    canvas = body((255, 255, 255), (238, 238, 243))
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle((26, 26, 102, 102), radius=10, fill=(255, 255, 255, 255), outline=(210, 210, 216, 255), width=2)
    for i in range(5):
        d.rounded_rectangle((38, 40 + i * 12, 78 - (i % 3) * 8, 43 + i * 12), radius=2, fill=(120, 120, 128, 255))
    d.polygon([(82, 82), (96, 96), (76, 100), (70, 88)], fill=(255, 159, 10, 255))
    return canvas


def notes_icon() -> Image.Image:
    canvas = body((255, 255, 255), (244, 244, 247))
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle((24, 24, 104, 104), radius=10, fill=(255, 255, 255, 255))
    d.rounded_rectangle((24, 24, 104, 50), radius=10, fill=(255, 214, 10, 255))
    d.rectangle((24, 42, 104, 50), fill=(255, 214, 10, 255))
    for i in range(4):
        d.rounded_rectangle((34, 60 + i * 11, 94 - (i % 2) * 10, 63 + i * 11), radius=2, fill=(150, 150, 158, 255))
    return canvas


def calendar_icon() -> Image.Image:
    canvas = body((255, 255, 255), (240, 240, 245))
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle((24, 24, 104, 104), radius=10, fill=(255, 255, 255, 255))
    d.rounded_rectangle((24, 24, 104, 52), radius=10, fill=(255, 59, 48, 255))
    d.rectangle((24, 42, 104, 52), fill=(255, 59, 48, 255))
    f_small = font(13, 700)
    f_day = font(48, 600)
    d.text((64, 38), "NOV", font=f_small, fill=(255, 255, 255, 255), anchor="mm")
    d.text((64, 78), "20", font=f_day, fill=(28, 28, 30, 255), anchor="mm")
    return canvas


def photos_icon() -> Image.Image:
    canvas = body((255, 255, 255), (242, 242, 247))
    d = ImageDraw.Draw(canvas)
    colors = [
        (255, 214, 10), (255, 149, 0), (255, 59, 48), (255, 45, 85),
        (175, 82, 222), (0, 122, 255), (90, 200, 250), (52, 199, 89),
    ]
    cx = cy = 64
    for i, color in enumerate(colors):
        angle = i * math.tau / len(colors)
        points = []
        for spread in (-0.30, 0.30):
            a = angle + spread
            points.append((cx + math.cos(a) * 42, cy + math.sin(a) * 42))
        points.append((cx, cy))
        d.polygon(points, fill=color + (255,))
    d.ellipse((54, 54, 74, 74), fill=(255, 255, 255, 255))
    return canvas


BUILDERS = [
    terminal_icon, files_icon, settings_icon, calculator_icon, monitor_icon,
    text_icon, notes_icon, calendar_icon, photos_icon,
]


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    blob = bytearray()
    sheet = Image.new("RGBA", (SIZE * len(BUILDERS), SIZE), (245, 245, 247, 255))
    for index, builder in enumerate(BUILDERS):
        icon = builder()
        sheet.alpha_composite(icon, (index * SIZE, 0))
        for y in range(SIZE):
            for x in range(SIZE):
                r, g, b, a = icon.getpixel((x, y))
                blob += bytes((r, g, b, a))
    (OUT / "app_icons.bin").write_bytes(bytes(blob))
    sheet.resize((SIZE * len(BUILDERS) // 2, SIZE // 2), Image.LANCZOS).save(ROOT / "build" / "icon-sheet.png")
    RUST.write_text(
        "\n".join(
            [
                "// @generated by tools/icongen.py. Do not edit by hand.",
                f"pub const ICON_SIZE: i32 = {SIZE};",
                f"pub const ICON_COUNT: usize = {len(BUILDERS)};",
                'pub static ICONS: &[u8] = include_bytes!("../../../assets/branding/app_icons.bin");',
                "",
            ]
        )
    )
    print(f"icons: {len(BUILDERS)} at {SIZE}x{SIZE} -> {OUT / 'app_icons.bin'}")
    print(f"sheet: {ROOT / 'build' / 'icon-sheet.png'}")
    print(f"rust:  {RUST}")


if __name__ == "__main__":
    main()
