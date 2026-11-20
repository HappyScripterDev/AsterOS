"""Preview the new light wallpaper generator on the host (Pillow)."""
from __future__ import annotations
import math
from pathlib import Path
from PIL import Image, ImageFilter

W, H = 640, 400
STYLES = {
    "sonoma": {
        "top": (247, 241, 232), "bottom": (203, 191, 177),
        "ribbons": [
            (0.30, 0.16, 1.1, 2.3, 0.00, 0.16, (110, 106, 115), 0.55),
            (0.44, 0.14, 1.6, 2.9, 1.20, 0.13, (244, 235, 222), 0.95),
            (0.58, 0.18, 0.9, 2.1, 2.30, 0.17, (196, 178, 157), 0.85),
            (0.74, 0.15, 1.4, 3.1, 0.70, 0.12, (250, 246, 239), 0.95),
        ],
    },
    "sequoia": {
        "top": (226, 244, 231), "bottom": (150, 198, 170),
        "ribbons": [
            (0.30, 0.16, 1.0, 2.2, 0.30, 0.17, (38, 92, 80), 0.45),
            (0.45, 0.16, 1.4, 2.7, 1.60, 0.15, (214, 241, 219), 0.95),
            (0.60, 0.18, 0.8, 1.9, 2.40, 0.16, (85, 160, 120), 0.70),
            (0.78, 0.14, 1.5, 3.2, 0.90, 0.12, (246, 252, 246), 0.95),
        ],
    },
    "ventura": {
        "top": (253, 231, 242), "bottom": (190, 178, 232),
        "ribbons": [
            (0.28, 0.18, 1.2, 2.6, 0.50, 0.16, (92, 103, 190), 0.50),
            (0.44, 0.16, 1.5, 2.4, 1.70, 0.15, (255, 236, 246), 0.95),
            (0.58, 0.19, 0.9, 2.1, 2.60, 0.17, (196, 138, 210), 0.65),
            (0.76, 0.14, 1.6, 3.3, 1.10, 0.12, (255, 248, 252), 0.95),
        ],
    },
    "monterey": {
        "top": (231, 244, 252), "bottom": (150, 188, 220),
        "ribbons": [
            (0.30, 0.17, 1.0, 2.4, 0.20, 0.16, (30, 74, 118), 0.50),
            (0.46, 0.15, 1.4, 2.8, 1.40, 0.14, (226, 243, 252), 0.95),
            (0.60, 0.18, 0.9, 2.0, 2.70, 0.16, (105, 163, 208), 0.65),
            (0.78, 0.13, 1.5, 3.0, 0.80, 0.12, (247, 252, 255), 0.95),
        ],
    },
}


def smoothstep(e0, e1, x):
    t = max(0.0, min(1.0, (x - e0) / (e1 - e0)))
    return t * t * (3 - 2 * t)


def paint(style):
    cfg = STYLES[style]
    img = Image.new("RGB", (W, H))
    px = img.load()
    top = cfg["top"]; bottom = cfg["bottom"]
    for y in range(H):
        t = y / (H - 1)
        base = tuple(top[i] + (bottom[i] - top[i]) * t for i in range(3))
        for x in range(W):
            nx = x / W
            ny = y / H
            r, g, b = base
            # broad top-left light
            light = max(0.0, 1.0 - math.hypot(nx - 0.2, ny - 0.12) * 1.35)
            r += light * 14; g += light * 12; b += light * 10
            for (offset, amp, f1, f2, phase, width, color, strength) in cfg["ribbons"]:
                curve = offset + amp * math.sin(nx * f1 * math.tau + phase) + amp * 0.45 * math.sin(nx * f2 * math.tau + phase * 1.7)
                d = (ny - curve) / width
                weight = 1.0 / (1.0 + d * d * d * d * d * d)
                if weight > 0.003:
                    w = weight * strength
                    r += (color[0] - r) * w
                    g += (color[1] - g) * w
                    b += (color[2] - b) * w
                # ribbon highlight along the upper edge
                edge = 1.0 / (1.0 + ((d + 0.55) / 0.12) ** 4)
                if edge > 0.01:
                    r += (255 - r) * edge * 0.20
                    g += (255 - g) * edge * 0.20
                    b += (255 - b) * edge * 0.20
            # vignette
            v = 1.0 - 0.13 * ((nx - 0.5) ** 2 + (ny - 0.5) ** 2) * 2.0
            px[x, y] = (int(max(0, min(255, r * v))), int(max(0, min(255, g * v))), int(max(0, min(255, b * v))))
    # subtle grain
    for y in range(H):
        for x in range(W):
            r, g, b = px[x, y]
            n = ((x * 7919 + y * 104729 + (x ^ y) * 31) % 5) - 2
            px[x, y] = (max(0, min(255, r + n)), max(0, min(255, g + n)), max(0, min(255, b + n)))
    return img


for name in STYLES:
    im = paint(name).resize((1280, 800), Image.LANCZOS)
    im.save(f"build/wall-{name}.png")
    print(name)
