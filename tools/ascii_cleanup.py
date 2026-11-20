"""Normalise typographic characters to ASCII (the UIfont atlas is ASCII-only)."""

from pathlib import Path

FILES = [
    "user/asterdesk/src/desktop.rs",
    "user/asterdesk/src/apps.rs",
    "user/asterdesk/src/ui.rs",
    "user/asterdesk/src/wm.rs",
    "user/asterdesk/src/theme.rs",
]

REPLACEMENTS = {
    0x00B7: "-",
    0x2026: "...",
    0x00D7: "x",
    0x2013: "-",
    0x2014: "-",
    0x2022: "-",
    0x201C: '"',
    0x201D: '"',
    0x2018: "'",
    0x2019: "'",
}

for name in FILES:
    data = Path(name).read_bytes()
    text = data.decode("utf-8")
    bad = sorted({c for c in text if ord(c) > 127})
    if not bad:
        continue
    print(f"{name}: replacing {[hex(ord(c)) for c in bad]}")
    for code, replacement in REPLACEMENTS.items():
        text = text.replace(chr(code), replacement)
    left = sorted({c for c in text if ord(c) > 127})
    if left:
        print(f"  still non-ASCII: {left}")
    Path(name).write_bytes(text.encode("utf-8"))
print("ascii scan done")
