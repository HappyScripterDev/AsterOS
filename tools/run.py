#!/usr/bin/env python3
"""Launch Aster OS in QEMU with reproducible arguments.

Reference test machine:
  q35, 512 MiB RAM, 1 vCPU, UEFI (OVMF), std VGA (GOP framebuffer),
  PS/2 keyboard + mouse, serial on COM1, GPT/FAT32 disk image.

Examples:
  python tools/run.py                                   # interactive
  python tools/run.py --headless --timeout 12            # boot + serial log
  python tools/run.py --headless --screenshot build/screen.ppm --timeout 12
  python tools/run.py --debug                           # gdb stub on :1234
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMAGE = ROOT / "build" / "aster.img"
SERIAL_LOG = ROOT / "build" / "serial.log"


def find_qemu() -> str:
    env = os.environ.get("QEMU")
    if env:
        return env
    found = shutil.which("qemu-system-x86_64")
    if found:
        return found
    for candidate in (
        Path(r"C:\Program Files\qemu\qemu-system-x86_64.exe"),
        Path(r"C:\Program Files (x86)\qemu\qemu-system-x86_64.exe"),
        Path("/usr/bin/qemu-system-x86_64"),
        Path("/usr/local/bin/qemu-system-x86_64"),
        Path("/opt/homebrew/bin/qemu-system-x86_64"),
    ):
        if candidate.exists():
            return str(candidate)
    sys.exit("error: qemu-system-x86_64 not found. Install QEMU 8+ and retry.")


def find_ovmf() -> Path:
    """Return a 4 MiB OVMF pflash image (vars + code), building it if needed.

    The QEMU Windows distribution ships split code/vars descriptors that tile a
    single 4 MiB flash chip. OVMF is mapped at 0xFFC00000 with the variable
    store at the base, so the combined image is vars.fd followed by code.fd.
    """
    env = os.environ.get("OVMF_CODE")
    if env and Path(env).exists():
        return Path(env)
    candidates = [
        ROOT / "tools" / "firmware" / "edk2-stable202605-r1-bin" / "x64",
        Path(r"C:\Program Files\qemu\share"),
        Path("/usr/share/OVMF"),
        Path("/usr/share/edk2/ovmf"),
    ]
    combined = ROOT / "build" / "OVMF.fd"
    for directory in candidates:
        code = directory / "code.fd" if directory.name.startswith("x64") else directory / "edk2-x86_64-code.fd"
        vars_file = directory / "vars.fd" if directory.name.startswith("x64") else directory / "edk2-i386-vars.fd"
        if not vars_file.exists():
            vars_file = directory / "edk2-x86_64-vars.fd"
        if code.exists() and vars_file.exists():
            data = vars_file.read_bytes() + code.read_bytes()
            if len(data) == 4 * 1024 * 1024:
                combined.parent.mkdir(exist_ok=True)
                combined.write_bytes(data)
                return combined
    for candidate in (
        Path("/usr/share/OVMF/OVMF_CODE.fd"),
        Path("/usr/share/edk2/ovmf/OVMF_CODE.fd"),
        Path("/opt/homebrew/share/qemu/edk2-x86_64-code.fd"),
    ):
        if candidate.exists():
            return candidate
    sys.exit("error: UEFI firmware not found. Install OVMF or set OVMF_CODE.")


class Qmp:
    def __init__(self, port: int, timeout: float = 10.0):
        deadline = time.time() + timeout
        self.sock = None
        while time.time() < deadline:
            try:
                self.sock = socket.create_connection(("127.0.0.1", port), timeout=1.0)
                break
            except OSError:
                time.sleep(0.1)
        if self.sock is None:
            raise RuntimeError("could not connect to QEMU QMP socket")
        self.file = self.sock.makefile("rwb")
        self._read()  # greeting
        self.command("qmp_capabilities")

    def _read(self) -> dict:
        line = self.file.readline()
        if not line:
            raise RuntimeError("QMP connection closed")
        return json.loads(line)

    def command(self, name: str, **arguments) -> dict:
        payload = {"execute": name}
        if arguments:
            payload["arguments"] = arguments
        self.file.write((json.dumps(payload) + "\n").encode())
        self.file.flush()
        while True:
            reply = self._read()
            if "event" in reply:
                continue
            return reply

    def close(self) -> None:
        try:
            self.file.close()
            self.sock.close()
        except Exception:
            pass


def main() -> None:
    parser = argparse.ArgumentParser(description="Run Aster OS under QEMU")
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--screenshot", help="capture a PPM screenshot before quitting")
    parser.add_argument("--timeout", type=float, default=12.0, help="seconds to run")
    parser.add_argument("--memory", default="512M")
    parser.add_argument("--debug", action="store_true", help="-S -s gdb stub")
    parser.add_argument("--iso", help="boot an ISO instead of the disk image")
    parser.add_argument("--serial", default="file:build/serial.log")
    parser.add_argument("--display", default=None)
    parser.add_argument("--qmp-port", type=int, default=4444)
    args = parser.parse_args()

    if not args.iso and not IMAGE.exists():
        sys.exit("error: build/aster.img missing. Run: python tools/build.py")

    SERIAL_LOG.parent.mkdir(exist_ok=True)
    qemu = find_qemu()
    firmware = find_ovmf()
    cmd = [
        qemu,
        "-machine", "q35",
        "-cpu", "max",
        "-smp", "1",
        "-m", args.memory,
        "-drive", f"if=pflash,format=raw,readonly=on,file={firmware}",
        "-device", "VGA",
        "-rtc", "base=localtime",
        "-no-reboot",
        "-serial", args.serial,
        "-monitor", "none",
    ]
    if args.iso:
        cmd += ["-cdrom", args.iso, "-boot", "d"]
    else:
        cmd += ["-drive", f"file={IMAGE},format=raw,if=ide,index=0"]
    if args.headless:
        cmd += [
            "-display", "none",
            "-qmp", f"tcp:127.0.0.1:{args.qmp_port},server=on,wait=off",
        ]
    elif args.display:
        cmd += ["-display", args.display]
    if args.debug:
        cmd += ["-S", "-s"]

    print("  $ " + " ".join(cmd))
    proc = subprocess.Popen(cmd, cwd=ROOT)
    if not args.headless:
        returncode = proc.wait()
        sys.exit(returncode)

    try:
        qmp = Qmp(args.qmp_port)
        time.sleep(max(0.0, args.timeout))
        if args.screenshot:
            shot = (ROOT / args.screenshot).resolve() if not os.path.isabs(args.screenshot) else Path(args.screenshot)
            shot.parent.mkdir(parents=True, exist_ok=True)
            reply = qmp.command("screendump", filename=str(shot))
            if "error" in reply:
                print(f"warning: screendump failed: {reply['error']}")
            else:
                print(f"screenshot: {shot} ({shot.stat().st_size} bytes)" if shot.exists() else "screenshot requested")
        qmp.command("quit")
        qmp.close()
    except Exception as exc:
        print(f"warning: {exc}")
    finally:
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    if SERIAL_LOG.exists():
        text = SERIAL_LOG.read_text(errors="replace")
        print(f"serial log ({len(text)} bytes): {SERIAL_LOG}")
        if text:
            print("--- serial ---")
            print(text[-8000:])


if __name__ == "__main__":
    main()
