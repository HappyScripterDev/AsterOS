#!/usr/bin/env python3
"""Account/identity core tests (aster-auth host suite)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from build import find_cargo  # noqa: E402


def main() -> int:
    result = subprocess.run([find_cargo(), "test", "-p", "aster-auth", "--quiet"])
    if result.returncode == 0:
        print("PASS: account validation, PBKDF2-HMAC-SHA256 and account DB tests")
    else:
        print("FAIL: account core tests")
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
