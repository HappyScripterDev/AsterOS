#!/usr/bin/env python3
"""Reproducible GPT + FAT32 disk image builder for Aster OS.

Pure standard library, deterministic output (fixed GUIDs, fixed timestamps).
The image contains a single EFI System Partition. Files are addressed with
8.3 names because the boot path deliberately avoids long-name parsing; the
kernel's own filesystem supports long names.

Usage (library):
    from image import build_image
    build_image("build/aster.img", {"/EFI/BOOT/BOOTX64.EFI": b"..."})
"""

from __future__ import annotations

import os
import struct
import zlib
from dataclasses import dataclass, field

SECTOR = 512
IMAGE_BYTES = 512 * 1024 * 1024
PART_START_LBA = 2048
SECTORS_PER_CLUSTER = 8
RESERVED_SECTORS = 32
NUM_FATS = 2
VOLUME_LABEL = b"ASTER OS   "
DATA_VOLUME_LABEL = b"ASTERDATA  "

ESP_TYPE_GUID = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
DATA_TYPE_GUID = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7"
DISK_GUID = "41535445-5220-4F53-8000-000000000001"
PART_GUID = "41535445-5220-4F53-8000-000000000002"
DATA_DISK_GUID = "41535445-5220-4F53-8000-000000000010"
DATA_PART_GUID = "41535445-5220-4F53-8000-000000000011"


def guid_bytes(text: str) -> bytes:
    """Encode a GUID string using the GPT mixed-endian layout."""
    a, b, c, d, e = text.split("-")
    return (
        struct.pack("<I", int(a, 16))
        + struct.pack("<H", int(b, 16))
        + struct.pack("<H", int(c, 16))
        + bytes.fromhex(d)
        + bytes.fromhex(e)
    )


def crc32(data: bytes) -> int:
    return zlib.crc32(data) & 0xFFFFFFFF


def dir_entry_83(name: str, attr: int, cluster: int, size: int) -> bytes:
    """Build a 32-byte FAT directory entry. Name must already be 8.3."""
    if name in (".", ".."):
        raw = name.ljust(11).encode("ascii")
    else:
        base, _, ext = name.partition(".")
        base = base.upper().ljust(8)[:8]
        ext = ext.upper().ljust(3)[:3]
        raw = (base + ext).encode("ascii")
    assert len(raw) == 11, name
    ent = bytearray(32)
    ent[0:11] = raw
    ent[11] = attr
    # Fixed DOS timestamp: 2026-01-01 00:00:00 (deterministic builds).
    struct.pack_into("<H", ent, 22, 0)  # time
    struct.pack_into("<H", ent, 24, (46 << 9) | (1 << 5) | 1)  # 2026-01-01
    struct.pack_into("<H", ent, 20, cluster >> 16)
    struct.pack_into("<H", ent, 26, cluster & 0xFFFF)
    struct.pack_into("<I", ent, 28, size)
    return bytes(ent)


@dataclass
class Node:
    name: str
    is_dir: bool
    data: bytes = b""
    children: list["Node"] = field(default_factory=list)
    cluster: int = 0
    size: int = 0


def build_tree(files: dict[str, bytes]) -> Node:
    root = Node("/", True)
    for path, data in files.items():
        parts = [p for p in path.split("/") if p]
        if not parts:
            continue
        node = root
        for part in parts[:-1]:
            existing = next((c for c in node.children if c.name == part and c.is_dir), None)
            if existing is None:
                existing = Node(part, True)
                node.children.append(existing)
            node = existing
        node.children.append(Node(parts[-1], False, data))
    root.children.sort(key=lambda n: (not n.is_dir, n.name))
    for node in root.children:
        node.children.sort(key=lambda n: (not n.is_dir, n.name))
    return root


@dataclass
class FatLayout:
    total_sectors: int
    fat_sectors: int
    clusters: int

    @staticmethod
    def compute(total_sectors: int, spc: int = SECTORS_PER_CLUSTER) -> "FatLayout":
        fat_sectors = 1
        for _ in range(8):
            data = total_sectors - RESERVED_SECTORS - NUM_FATS * fat_sectors
            clusters = data // spc
            need = ((clusters + 2) * 4 + SECTOR - 1) // SECTOR
            if need == fat_sectors:
                break
            fat_sectors = need
        data = total_sectors - RESERVED_SECTORS - NUM_FATS * fat_sectors
        clusters = data // spc
        return FatLayout(total_sectors, fat_sectors, clusters)


class Fat32Writer:
    def __init__(
        self,
        f,
        part_offset: int,
        total_sectors: int,
        spc: int = SECTORS_PER_CLUSTER,
        label: bytes = VOLUME_LABEL,
    ):
        self.f = f
        self.off = part_offset
        self.spc = spc
        self.label = label
        self.layout = FatLayout.compute(total_sectors, spc)
        assert self.layout.clusters >= 65525, "volume too small for FAT32"
        self.fat = [0] * (self.layout.clusters + 2)
        self.fat[0] = 0x0FFFFFF8
        self.fat[1] = 0x0FFFFFFF
        self.next_cluster = 2

    def alloc(self, count: int = 1) -> int:
        if count <= 0:
            count = 1
        start = self.next_cluster
        if start + count > self.layout.clusters + 2:
            raise RuntimeError("disk image full")
        for i in range(count):
            self.fat[start + i] = start + i + 1 if i + 1 < count else 0x0FFFFFFF
        self.next_cluster += count
        return start

    def cluster_offset(self, cluster: int) -> int:
        first_data = RESERVED_SECTORS + NUM_FATS * self.layout.fat_sectors
        return self.off + (first_data + (cluster - 2) * self.spc) * SECTOR

    def write_at(self, offset: int, data: bytes) -> None:
        self.f.seek(offset)
        self.f.write(data)

    def format_boot_sector(self) -> None:
        bs = bytearray(SECTOR)
        bs[0:3] = b"\xEB\x58\x90"
        bs[3:11] = b"ASTEROS "
        struct.pack_into("<H", bs, 11, SECTOR)
        bs[13] = self.spc
        struct.pack_into("<H", bs, 14, RESERVED_SECTORS)
        bs[16] = NUM_FATS
        struct.pack_into("<H", bs, 17, 0)
        struct.pack_into("<H", bs, 19, 0)
        bs[21] = 0xF8
        struct.pack_into("<H", bs, 22, 0)
        struct.pack_into("<H", bs, 24, 32)
        struct.pack_into("<H", bs, 26, 64)
        struct.pack_into("<I", bs, 28, 0)
        struct.pack_into("<I", bs, 32, self.layout.total_sectors)
        struct.pack_into("<I", bs, 36, self.layout.fat_sectors)
        struct.pack_into("<H", bs, 40, 0)
        struct.pack_into("<H", bs, 42, 0)
        struct.pack_into("<I", bs, 44, 2)
        struct.pack_into("<H", bs, 48, 1)
        struct.pack_into("<H", bs, 50, 6)
        bs[64] = 0x80
        bs[66] = 0x29
        struct.pack_into("<I", bs, 67, 0x41535445)
        bs[71:82] = self.label.ljust(11)[:11]
        bs[82:90] = b"FAT32   "
        bs[510:512] = b"\x55\xAA"
        self.write_at(self.off, bytes(bs))

        fsinfo = bytearray(SECTOR)
        struct.pack_into("<I", fsinfo, 0, 0x41615252)
        struct.pack_into("<I", fsinfo, 484, 0x61417272)
        struct.pack_into("<I", fsinfo, 488, 0xFFFFFFFF)
        struct.pack_into("<I", fsinfo, 492, 0xFFFFFFFF)
        struct.pack_into("<I", fsinfo, 508, 0xAA550000)
        self.write_at(self.off + SECTOR, bytes(fsinfo))
        self.write_at(self.off + 6 * SECTOR, bytes(bs))
        self.write_at(self.off + 7 * SECTOR, bytes(fsinfo))

    def flush_fat(self) -> None:
        raw = b"".join(struct.pack("<I", e) for e in self.fat)
        raw = raw.ljust(self.layout.fat_sectors * SECTOR, b"\x00")
        for i in range(NUM_FATS):
            self.write_at(
                self.off + (RESERVED_SECTORS + i * self.layout.fat_sectors) * SECTOR, raw
            )

    def write_dir(self, node: Node, parent_cluster: int) -> None:
        entries = [dir_entry_83(".", 0x10, node.cluster, 0)]
        entries.append(dir_entry_83("..", 0x10, parent_cluster, 0))
        for child in node.children:
            attr = 0x10 if child.is_dir else 0x20
            size = 0 if child.is_dir else child.size
            entries.append(dir_entry_83(child.name, attr, child.cluster, size))
        assert len(entries) <= 128, f"directory {node.name} has too many entries"
        blob = b"".join(entries).ljust(self.spc * SECTOR, b"\x00")
        self.write_at(self.cluster_offset(node.cluster), blob)


def alloc_tree(fat: Fat32Writer, node: Node) -> None:
    if node.is_dir:
        node.cluster = fat.alloc(1)
        for child in node.children:
            alloc_tree(fat, child)
    else:
        node.size = len(node.data)
        clusters = max(1, (len(node.data) + fat.spc * SECTOR - 1)
                       // (fat.spc * SECTOR))
        node.cluster = fat.alloc(clusters)


def emit_tree(fat: Fat32Writer, node: Node, parent_cluster: int) -> None:
    if node.is_dir:
        fat.write_dir(node, parent_cluster)
        for child in node.children:
            emit_tree(fat, child, node.cluster)
    else:
        # Cluster chains are consecutive by construction.
        fat.write_at(fat.cluster_offset(node.cluster), node.data)


def build_image(
    path: str,
    files: dict[str, bytes],
    image_bytes: int = IMAGE_BYTES,
    *,
    sectors_per_cluster: int = SECTORS_PER_CLUSTER,
    volume_label: bytes = VOLUME_LABEL,
    partition_type_guid: str = ESP_TYPE_GUID,
    partition_guid: str = PART_GUID,
    disk_guid: str = DISK_GUID,
    partition_name: str = "Aster ESP",
) -> None:
    """Create a GPT disk image containing one FAT32 partition with `files`."""
    total_sectors = image_bytes // SECTOR
    part_sectors = total_sectors - PART_START_LBA - 34
    if total_sectors < 4096:
        raise ValueError("image too small")

    # Build FAT partition in place.
    with open(path, "wb") as f:
        f.truncate(total_sectors * SECTOR)
        fat = Fat32Writer(
            f,
            PART_START_LBA * SECTOR,
            part_sectors,
            spc=sectors_per_cluster,
            label=volume_label,
        )
        fat.format_boot_sector()

        targets = {("/" + k.lstrip("/")): v for k, v in files.items()}
        # Normalize to 8.3-compatible paths.
        for name in targets:
            for comp in name.strip("/").split("/"):
                assert len(comp) <= 12, f"path component too long for 8.3: {comp}"
        root = build_tree(targets)
        alloc_tree(fat, root)
        fat.flush_fat()
        emit_tree(fat, root, root.cluster)

        # Protective MBR
        mbr = bytearray(SECTOR)
        mbr[440:444] = b"\x00\x00\x00\x00"
        mbr[446] = 0x00
        mbr[447:450] = b"\x00\x02\x00"
        mbr[450] = 0xEE
        mbr[451:454] = b"\xFF\xFF\xFF"
        struct.pack_into("<I", mbr, 454, 1)
        struct.pack_into("<I", mbr, 458, min(total_sectors - 1, 0xFFFFFFFF))
        mbr[510:512] = b"\x55\xAA"
        f.seek(0)
        f.write(mbr)

        # Primary GPT
        entries = bytearray(128 * 128)
        entry = bytearray(128)
        entry[0:16] = guid_bytes(partition_type_guid)
        entry[16:32] = guid_bytes(partition_guid)
        struct.pack_into("<Q", entry, 32, PART_START_LBA)
        struct.pack_into("<Q", entry, 40, PART_START_LBA + part_sectors - 1)
        struct.pack_into("<Q", entry, 48, 0)
        name = partition_name.encode("utf-16-le")[:72]
        entry[56:56 + len(name)] = name
        entries[0:128] = entry

        header = bytearray(SECTOR)
        header[0:8] = b"EFI PART"
        struct.pack_into("<I", header, 8, 0x00010000)
        struct.pack_into("<I", header, 12, 92)
        struct.pack_into("<I", header, 16, 0)  # CRC placeholder
        struct.pack_into("<I", header, 20, 0)
        struct.pack_into("<Q", header, 24, 1)
        struct.pack_into("<Q", header, 32, total_sectors - 1)
        struct.pack_into("<Q", header, 40, 34)
        struct.pack_into("<Q", header, 48, total_sectors - 34)
        header[56:72] = guid_bytes(disk_guid)
        struct.pack_into("<Q", header, 72, 2)
        struct.pack_into("<I", header, 80, 128)
        struct.pack_into("<I", header, 84, 128)
        struct.pack_into("<I", header, 88, crc32(bytes(entries)))
        header_check = bytearray(header[:92])
        struct.pack_into("<I", header_check, 16, 0)
        struct.pack_into("<I", header, 16, crc32(bytes(header_check)))

        f.seek(1 * SECTOR)
        f.write(header)
        f.seek(2 * SECTOR)
        f.write(entries)

        backup_entries_lba = total_sectors - 33
        f.seek(backup_entries_lba * SECTOR)
        f.write(entries)
        backup = bytearray(header)
        struct.pack_into("<Q", backup, 24, total_sectors - 1)
        struct.pack_into("<Q", backup, 32, 1)
        struct.pack_into("<Q", backup, 72, backup_entries_lba)
        struct.pack_into("<I", backup, 16, 0)
        backup_check = bytearray(backup[:92])
        struct.pack_into("<I", backup_check, 16, 0)
        struct.pack_into("<I", backup, 16, crc32(bytes(backup_check)))
        f.seek((total_sectors - 1) * SECTOR)
        f.write(backup)


def build_data_image(path: str, files: dict[str, bytes] | None = None) -> None:
    """Create the 512 MiB persistent data volume (GPT + one FAT32 partition).

    It is deliberately a separate disk from the boot image: the kernel never
    writes the ESP, and rebuilding Aster cannot destroy user data.
    """
    build_image(
        path,
        files or {},
        sectors_per_cluster=8,
        volume_label=DATA_VOLUME_LABEL,
        partition_type_guid=DATA_TYPE_GUID,
        partition_guid=DATA_PART_GUID,
        disk_guid=DATA_DISK_GUID,
        partition_name="Aster Data",
    )


def verify_image(path: str, expected: dict[str, bytes]) -> list[str]:
    """Parse the image back and compare every expected file. Returns problems."""
    problems: list[str] = []
    with open(path, "rb") as f:
        mbr = f.read(SECTOR)
        if mbr[510:512] != b"\x55\xAA":
            problems.append("missing MBR signature")
        f.seek(SECTOR)
        header = f.read(SECTOR)
        if header[0:8] != b"EFI PART":
            problems.append("bad GPT signature")
            return problems
        size = struct.unpack_from("<I", header, 12)[0]
        stored = struct.unpack_from("<I", header, 16)[0]
        check = bytearray(header[:size])
        struct.pack_into("<I", check, 16, 0)
        if crc32(bytes(check)) != stored:
            problems.append("GPT header CRC mismatch")
        entries_lba = struct.unpack_from("<Q", header, 72)[0]
        f.seek(entries_lba * SECTOR)
        entries = f.read(128 * 128)
        part_lba = struct.unpack_from("<Q", entries, 32)[0]
        part_sectors = struct.unpack_from("<Q", entries, 40)[0] - part_lba + 1

        base = part_lba * SECTOR

        def rd(off: int, n: int) -> bytes:
            f.seek(base + off)
            return f.read(n)

        bs = rd(0, SECTOR)
        if bs[510:512] != b"\x55\xAA":
            problems.append("missing FAT boot signature")
            return problems
        bps = struct.unpack_from("<H", bs, 11)[0]
        spc = bs[13]
        reserved = struct.unpack_from("<H", bs, 14)[0]
        nfats = bs[16]
        fat_sectors = struct.unpack_from("<I", bs, 36)[0]
        root_cluster = struct.unpack_from("<I", bs, 44)[0]
        if bps != SECTOR:
            problems.append(f"unexpected sector size {bps}")
        cluster_bytes = spc * bps
        first_data = reserved + nfats * fat_sectors

        fat_raw = rd(reserved * bps, fat_sectors * bps)
        fat = list(struct.unpack("<%dI" % (len(fat_raw) // 4), fat_raw))

        def chain(start: int):
            out, c, guard = [], start, 0
            while 2 <= c < len(fat) and guard < 100000:
                out.append(c)
                nxt = fat[c] & 0x0FFFFFFF
                if nxt >= 0x0FFFFFF8:
                    break
                c = nxt
                guard += 1
            return out

        def cluster_data(c: int) -> bytes:
            off = (first_data + (c - 2) * spc) * bps
            return rd(off, cluster_bytes)

        def read_file(start: int, size: int) -> bytes:
            return b"".join(cluster_data(c) for c in chain(start))[:size]

        def walk(cluster: int, prefix: str, seen: set) -> None:
            if cluster in seen:
                problems.append(f"directory loop at {prefix}")
                return
            seen.add(cluster)
            blob = b"".join(cluster_data(c) for c in chain(cluster))
            for i in range(0, len(blob), 32):
                ent = blob[i:i + 32]
                if len(ent) < 32 or ent[0] == 0:
                    break
                if ent[0] == 0xE5:
                    continue
                attr = ent[11]
                if attr & 0x0F == 0x0F:
                    continue
                name = ent[0:8].decode("ascii", "replace").rstrip()
                ext = ent[8:11].decode("ascii", "replace").rstrip()
                if name in (".", "..") or not name:
                    continue
                full = name + ("." + ext if ext else "")
                path = prefix + "/" + full
                hi = struct.unpack_from("<H", ent, 20)[0]
                lo = struct.unpack_from("<H", ent, 26)[0]
                start = (hi << 16) | lo
                size = struct.unpack_from("<I", ent, 28)[0]
                if attr & 0x10:
                    walk(start, path, seen)
                else:
                    expected_name = path.lower()
                    match = next(
                        (k for k in expected if ("/" + k.lstrip("/")).lower() == expected_name),
                        None,
                    )
                    if match is None:
                        problems.append(f"unexpected file {path}")
                        continue
                    actual = read_file(start, size)
                    if actual != expected[match]:
                        problems.append(f"content mismatch for {path}")
                    else:
                        problems.append(f"ok {path} ({size} bytes)")

        walk(root_cluster, "", set())
    for name in expected:
        expected_name = "/" + name.lstrip("/")
        if not any(p.startswith("ok " + expected_name) for p in problems):
            if not any(expected_name.lower() == ("/" + p.split(" ")[1]).lower()
                       for p in problems if p.startswith("ok ")):
                problems.append(f"missing file {expected_name}")
    return problems


if __name__ == "__main__":
    import sys

    out = sys.argv[1] if len(sys.argv) > 1 else "build/aster.img"
    demo = {"/EFI/BOOT/BOOTX64.EFI": b"demo"}
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    build_image(out, demo)
    for line in verify_image(out, demo):
        print(line)
