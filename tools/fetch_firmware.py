#!/usr/bin/env python3
"""Fetch a known-good OVMF build for QEMU (UEFI firmware for testing).

The firmware is a build-time/test-only dependency (BSD-2-Clause-Patent) and is
not part of the Aster OS image. Files are stored under tools/firmware/, which
is git-ignored.
"""

from __future__ import annotations

import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / "tools" / "firmware"
URL = (
    "https://github.com/rust-osdev/ovmf-prebuilt/releases/download/"
    "edk2-stable202605-r1/edk2-stable202605-r1-bin.tar.xz"
)
ARCHIVE = DEST / "ovmf.tar.xz"
EXPECTED = DEST / "edk2-stable202605-r1-bin" / "x64"


def main() -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    if (EXPECTED / "code.fd").exists() and (EXPECTED / "vars.fd").exists():
        print(f"firmware present: {EXPECTED}")
        return
    if not ARCHIVE.exists():
        print(f"downloading {URL}")
        urllib.request.urlretrieve(URL, ARCHIVE)
    print(f"extracting {ARCHIVE}")
    with tarfile.open(ARCHIVE, "r:xz") as archive:
        archive.extractall(DEST)
    if not (EXPECTED / "code.fd").exists():
        raise SystemExit(f"firmware extraction failed: {EXPECTED}")
    print(f"firmware ready: {EXPECTED}")
    print("run.py combines vars.fd + code.fd into a 4 MiB pflash image.")


if __name__ == "__main__":
    main()
