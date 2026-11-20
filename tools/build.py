#!/usr/bin/env python3
"""Aster OS build orchestrator.

Stages:
  1. shared ABI crate        (consumer of the rest)
  2. bootloader              x86_64-unknown-uefi
  3. user space              x86_64-unknown-none (flat user binaries)
  4. kernel                  x86_64-unknown-none (higher-half ELF)
  5. disk image              GPT + FAT32 ESP, verified by tools/image.py

The script works from any directory and is the single documented entry point.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
TARGET_DIR = ROOT / "target"
KERNEL_TARGET = "x86_64-unknown-none"
UEFI_TARGET = "x86_64-unknown-uefi"


def find_cargo() -> str:
    env = os.environ.get("CARGO")
    if env:
        return env
    # Prefer rustup's cargo: it owns the toolchain selected by
    # rust-toolchain.toml and the cross-compilation targets. A standalone
    # system Rust may exist without x86_64-unknown-none/uefi.
    home = Path.home() / ".cargo" / "bin"
    for name in ("cargo.exe", "cargo"):
        candidate = home / name
        if candidate.exists():
            # Keep rustup's bin directory first so it finds rustc/rust-lld.
            os.environ["PATH"] = str(home) + os.pathsep + os.environ.get("PATH", "")
            return str(candidate)
    found = shutil.which("cargo")
    if found:
        return found
    sys.exit("error: cargo not found. Install Rust from https://rustup.rs and retry.")


CARGO = find_cargo()
sys.path.insert(0, str(Path(__file__).resolve().parent))
from image import build_data_image, build_image, verify_image  # noqa: E402


def run(cmd: list[str], **kwargs) -> None:
    printable = " ".join(str(c) for c in cmd)
    print(f"  $ {printable}")
    result = subprocess.run(cmd, **kwargs)
    if result.returncode != 0:
        sys.exit(f"error: command failed ({result.returncode}): {printable}")


def cargo_build(package: str, target: str, extra: list[str] | None = None) -> None:
    cmd = [CARGO, "build", "--release", "-p", package, "--target", target]
    if extra:
        cmd += extra
    run(cmd, cwd=ROOT)


def artifact(package: str, target: str, name: str) -> Path:
    exe = name
    if target == UEFI_TARGET and not exe.endswith(".efi"):
        exe += ".efi"
    path = TARGET_DIR / target / "release" / exe
    if not path.exists():
        sys.exit(f"error: expected build artifact missing: {path}")
    return path


def build(clean: bool = False, modules: bool = True, desktop: bool = True, fresh_data: bool = False) -> None:
    start = time.time()
    BUILD.mkdir(exist_ok=True)
    if clean:
        shutil.rmtree(TARGET_DIR, ignore_errors=True)

    print("[1/5] shared ABI")
    cargo_build("aster-abi", KERNEL_TARGET)

    print("[2/5] Aster Boot (UEFI)")
    cargo_build("asterboot", UEFI_TARGET)
    boot_efi = artifact("asterboot", UEFI_TARGET, "asterboot.efi")

    files: dict[str, bytes] = {}
    files["/EFI/BOOT/BOOTX64.EFI"] = boot_efi.read_bytes()

    if desktop:
        print("[3/5] desktop (ring 3)")
        cargo_build("asterdesk", KERNEL_TARGET)
        desk = artifact("asterdesk", KERNEL_TARGET, "asterdesk")
        shutil.copyfile(desk, BUILD / "asterdesk.elf")
        files["/DESK.ELF"] = desk.read_bytes()

        print("[3/5] isolation probes")
        for package, image_name in (
            ("isoprobe", "ISOPROBE.ELF"),
            ("faultprobe", "FAULTPRB.ELF"),
            ("ipcprobe", "IPCPROBE.ELF"),
        ):
            cargo_build(package, KERNEL_TARGET)
            probe = artifact(package, KERNEL_TARGET, package)
            files[f"/{image_name}"] = probe.read_bytes()
    else:
        print("[3/5] desktop skipped")

    if modules:
        print("[4/5] kernel")
        # The kernel embeds nothing from user space; the bootloader passes
        # modules, so build order only matters for packaging.
        cargo_build("asterkernel", KERNEL_TARGET)
        kernel = artifact("asterkernel", KERNEL_TARGET, "asterkernel")
        shutil.copyfile(kernel, BUILD / "asterkernel.elf")
        files["/KERNEL.ELF"] = kernel.read_bytes()
    else:
        print("[4/5] kernel skipped")

    print("[5/5] disk image")
    img = BUILD / "aster.img"
    build_image(str(img), files)
    problems = verify_image(str(img), files)
    bad = [p for p in problems if not p.startswith("ok ")]
    for p in problems:
        if p.startswith("ok "):
            print(f"  verifies: {p[3:]}")
    if bad:
        for p in bad:
            print(f"  ERROR: {p}")
        sys.exit("error: disk image verification failed")
    print(f"  image: {img} ({img.stat().st_size // (1024 * 1024)} MiB)")

    # The data volume is user state, not build output. Create it once and keep
    # it across builds unless the caller explicitly asks for a fresh one.
    data = BUILD / "data.img"
    if fresh_data or not data.exists():
        # First-boot contents. They are written once: later builds keep the
        # volume so user edits survive.
        seeds = {
            "/WELCOME.TXT": (
                b"Welcome to Aster OS.\n\n"
                b"This home directory lives on the persistent data volume.\n"
                b"Files here survive a reboot.\n"
            ),
            "/SETTINGS.CNF": (
                b"theme=light\naccent=blue\nanimations=1.00\n"
                b"wallpaper=0\nreduce_motion=0\nbrightness=1.00\n"
            ),
            "/NOTES/IDEAS.TXT": (
                b"Ideas\n-----\n"
                b"- Native compositor with real window management\n"
                b"- Persistent home volume\n"
                b"- Process isolation and IPC\n"
            ),
            "/DOCS/README.TXT": b"Documents saved here persist across reboots.\n",
        }
        build_data_image(str(data), seeds)
        print(f"  data image: {data} ({data.stat().st_size // (1024 * 1024)} MiB, fresh)")
    else:
        print(f"  data image: {data} (kept)")

    print(f"build finished in {time.time() - start:.1f}s")


def main() -> None:
    parser = argparse.ArgumentParser(description="Build Aster OS")
    parser.add_argument("--clean", action="store_true", help="remove target/ first")
    parser.add_argument("--fresh-data", action="store_true", help="recreate build/data.img")
    parser.add_argument("--boot-only", action="store_true", help="only build the UEFI bootloader")
    args = parser.parse_args()
    if args.boot_only:
        cargo_build("asterboot", UEFI_TARGET)
        boot = artifact("asterboot", UEFI_TARGET, "asterboot.efi")
        img = BUILD / "aster.img"
        files = {"/EFI/BOOT/BOOTX64.EFI": boot.read_bytes()}
        build_image(str(img), files)
        print(f"boot-only image: {img}")
        return
    build(clean=args.clean, fresh_data=args.fresh_data)


if __name__ == "__main__":
    main()
