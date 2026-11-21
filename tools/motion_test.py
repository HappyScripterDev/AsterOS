#!/usr/bin/env python3
"""Runs the aster-motion host test suite (easing, tweens, springs, tracks)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from build import find_cargo  # noqa: E402


def main() -> int:
    result = subprocess.run([find_cargo(), "test", "-p", "aster-motion", "--quiet"])
    if result.returncode == 0:
        print("PASS: aster-motion host tests")
    else:
        print("FAIL: aster-motion host tests")
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
