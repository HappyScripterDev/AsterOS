"""Host-side FAT32 file extraction for tests (8.3 names)."""

from __future__ import annotations

import struct


def _crc32(data: bytes) -> int:
    import zlib

    return zlib.crc32(data) & 0xFFFFFFFF


def extract_file(path: str, wanted: str) -> bytes | None:
    """Return the contents of `wanted` from a GPT/FAT32 image, or None."""
    wanted = "/" + wanted.lstrip("/")

    with open(path, "rb") as f:
        mbr = f.read(512)
        if mbr[510:512] != b"\x55\xAA":
            return None
        f.seek(512)
        header = f.read(512)
        if header[0:8] != b"EFI PART":
            return None
        entries_lba = struct.unpack_from("<Q", header, 72)[0]
        f.seek(entries_lba * 512)
        entries = f.read(128 * 128)
        part_lba = struct.unpack_from("<Q", entries, 32)[0]
        base = part_lba * 512

        def read(offset: int, count: int) -> bytes:
            f.seek(base + offset)
            return f.read(count)

        boot = read(0, 512)
        spc = boot[13]
        reserved = struct.unpack_from("<H", boot, 14)[0]
        fats = boot[16]
        fat_sectors = struct.unpack_from("<I", boot, 36)[0]
        root = struct.unpack_from("<I", boot, 44)[0]
        cluster_bytes = spc * 512
        first_data = reserved + fats * fat_sectors
        fat_raw = read(reserved * 512, fat_sectors * 512)
        fat = list(struct.unpack("<%dI" % (len(fat_raw) // 4), fat_raw))

        def chain(start: int) -> list[int]:
            result, cluster = [], start
            while 2 <= cluster < len(fat) and len(result) < 100000:
                result.append(cluster)
                nxt = fat[cluster] & 0x0FFFFFFF
                if nxt >= 0x0FFFFFF8:
                    break
                cluster = nxt
            return result

        def cluster_data(cluster: int) -> bytes:
            offset = (first_data + (cluster - 2) * spc) * 512
            return read(offset, cluster_bytes)

        def file_data(start: int, size: int) -> bytes:
            return b"".join(cluster_data(c) for c in chain(start))[:size]

        def walk(cluster: int) -> bytes | None:
            for c in chain(cluster):
                blob = cluster_data(c)
                for i in range(0, len(blob), 32):
                    entry = blob[i : i + 32]
                    if len(entry) < 32 or entry[0] == 0:
                        return None
                    if entry[0] == 0xE5:
                        continue
                    attr = entry[11]
                    if attr & 0x0F == 0x0F or attr & 0x08:
                        continue
                    name = entry[0:8].decode("ascii", "replace").rstrip()
                    ext = entry[8:11].decode("ascii", "replace").rstrip()
                    full = name + ("." + ext if ext else "")
                    if full in (".", ".."):
                        continue
                    start = (struct.unpack_from("<H", entry, 20)[0] << 16) | struct.unpack_from(
                        "<H", entry, 26
                    )[0]
                    size = struct.unpack_from("<I", entry, 28)[0]
                    if attr & 0x10:
                        found = walk(start)
                        if found is not None:
                            return found
                    elif full.upper() == wanted.lstrip("/").upper():
                        return file_data(start, size)
            return None

        return walk(root)
