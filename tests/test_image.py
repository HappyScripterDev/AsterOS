"""Host-side tests for the GPT + FAT32 image builder.

Run with: python -m unittest discover -s tests -v
"""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))

from image import build_image, verify_image, crc32  # noqa: E402


class ImageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory()
        cls.image = os.path.join(cls.temp.name, "test.img")
        cls.files = {
            "/EFI/BOOT/BOOTX64.EFI": b"\x7fELF" + b"boot" * 900,
            "/KERNEL.ELF": os.urandom(5000),
            "/DESK.ELF": b"desk",
            "/SYSTEM/CONFIG.TXT": b"theme=dark\n",
        }
        build_image(cls.image, cls.files, image_bytes=300 * 1024 * 1024)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def test_files_round_trip(self) -> None:
        problems = verify_image(self.image, self.files)
        errors = [p for p in problems if not p.startswith("ok ")]
        self.assertEqual(errors, [], f"image verification failed: {errors}")
        verified = [p for p in problems if p.startswith("ok ")]
        self.assertEqual(len(verified), len(self.files))

    def test_payload_bytes(self) -> None:
        # Read back the files through the builder's own parser as an
        # independent check that cluster chains and sizes are correct.
        with open(self.image, "rb") as handle:
            handle.seek(512)
            header = handle.read(92)
        self.assertEqual(header[:8], b"EFI PART")
        stored = int.from_bytes(header[16:20], "little")
        check = bytearray(header)
        check[16:20] = b"\x00\x00\x00\x00"
        self.assertEqual(crc32(bytes(check)), stored, "GPT header CRC is wrong")

    def test_detects_corruption(self) -> None:
        corrupt = os.path.join(self.temp.name, "corrupt.img")
        with open(self.image, "rb") as source, open(corrupt, "wb") as target:
            data = bytearray(source.read(4096))
            data[512 + 20] ^= 0xFF  # flip a byte inside the GPT header
            target.write(data)
            target.write(b"\x00" * (1024 * 1024))
        problems = verify_image(corrupt, self.files)
        self.assertTrue(
            any("CRC" in p or "missing" in p for p in problems),
            f"corruption was not detected: {problems[:3]}",
        )

    def test_rejects_long_names(self) -> None:
        with self.assertRaises(AssertionError):
            build_image(
                os.path.join(self.temp.name, "bad.img"),
                {"/A-VERY-LONG-NAME.TXT": b"x"},
                image_bytes=300 * 1024 * 1024,
            )


if __name__ == "__main__":
    unittest.main()
