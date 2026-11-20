#!/usr/bin/env python3
"""Count source lines by category, excluding build artifacts and generated data."""

from __future__ import annotations

from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKIP = {"build", "target", ".git", "firmware", "__pycache__", "node_modules"}
CATEGORIES = {
    ".rs": "Rust",
    ".py": "Python",
    ".md": "Markdown",
    ".toml": "TOML",
    ".yml": "YAML",
    ".yaml": "YAML",
    ".ld": "Linker scripts",
    ".asm": "Assembly",
    ".json": "JSON",
    ".txt": "Text",
}


def main() -> None:
    counts = Counter()
    files = Counter()
    for path in ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP for part in path.parts):
            continue
        if path.name == "font_data.rs":
            continue
        category = CATEGORIES.get(path.suffix.lower())
        if not category:
            continue
        try:
            lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
        except OSError:
            continue
        counts[category] += len(lines)
        files[category] += 1
    total = sum(counts.values())
    for category, count in counts.most_common():
        print(f"{category:16s} {count:7d} lines in {files[category]:3d} files")
    print(f"{'TOTAL':16s} {total:7d} lines")


if __name__ == "__main__":
    main()
