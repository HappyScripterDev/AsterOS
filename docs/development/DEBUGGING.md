# Debugging Aster OS

## 1. Serial console is the primary diagnostic

Everything important is written to COM1 at 115200 8N1:

- Aster Boot: `BOOT-LOADER-START`, `[ok] ...`, `[fatal] ...`,
  `BOOT-HANDOFF-OK`.
- Kernel: timestamped `info/warn/error/debug` lines with subsystem prefixes.
- Desktop: `[user] desktop: ...` lines.

Run with a serial file and inspect the tail:

```bash
python tools/run.py --headless --timeout 12
cat build/serial.log
```

Useful markers:

```
BOOT-HANDOFF-OK
KERNEL-ENTRY-OK
[     0.000] info: pmm: 510 MiB usable, ...
[     0.000] info: KERNEL-READY
[user] desktop: DESKTOP-READY
[user] desktop: frame 1 at 1850 ms, ...
```

`grep -E "PANIC|fault|halted" build/serial.log` should return nothing on a good
boot.

## 2. Screenshots without a display

QEMU's QMP `screendump` works headless. `tools/run.py --headless --screenshot`
drives it; the scripts in `tools/` use the same mechanism. Convert PPM to PNG:

```bash
python -c "from PIL import Image; Image.open('build/screen.ppm').save('build/screen.png')"
```

The first frame is not available before the desktop reports
`DESKTOP-READY`, which takes about 4.3 s under QEMU TCG.

## 3. CPU faults and triple faults

Enable QEMU's interrupt log:

```bash
qemu-system-x86_64 ... -d int,cpu_reset -D build/qemu-int.log
```

Look at the last `check_exception` lines. In this project this log was used to
find a real bug: the idle thread was created with `CR3=0`, so the first
scheduler switch ran with no page tables and triple-faulted. The log showed
`CR3=0000000000000000` at the fault.

Common causes:

| Fault | Likely cause |
|---|---|
| Page fault in kernel with `CR2` = executing RIP | CR3 points at an incomplete address space, or the kernel image is not mapped |
| Page fault in user with error bit 0 | Page not present (bad ELF mapping or stack) |
| Page fault in user with error bit 1 | Present but not user-accessible; missing U/S on an intermediate table |
| General protection fault | Bad segment selector/TSS, non-canonical address, privileged instruction in ring 3 |
| Double fault | Broken IDT/TSS or a fault while delivering another fault; check IST1 |
| Triple fault / QEMU reset | Double fault with an invalid handler, invalid CR3, or an IDT that is not mapped |

The kernel prints all general registers and halts on fatal exceptions. It does
not reboot silently.

## 4. GDB

```bash
python tools/run.py --debug
# in another terminal (gdb with x86_64 support)
gdb build/asterkernel.elf
(gdb) target remote :1234
(gdb) hbreak kernel_main
(gdb) continue
```

The kernel is linked at `0xFFFFFFFF80000000`; GDB handles the high addresses
once the ELF symbols are loaded. Useful breakpoints: `kernel_main`, `schedule`,
`syscall::handle`, `user::spawn_module`, `exceptions::handle`.

## 5. Inspecting the image

```bash
python - <<'PY'
import sys; sys.path.insert(0, 'tools')
from pathlib import Path
from image import verify_image
for line in verify_image('build/aster.img', {'/KERNEL.ELF': Path('build/asterkernel.elf').read_bytes()}):
    print(line)
PY
```

The verifier prints `ok /path (bytes)` for each file or a specific error (CRC,
content mismatch, missing file). The build refuses to produce an invalid image.

## 6. Desktop performance

The desktop logs frame times:

```
[user] desktop: frame 60 at 5034 ms, 56 ms this frame, 54.0 ms avg
```

If frames are much slower:

- Check that the wallpaper and blur are generated at reduced resolution
  (`rebuild_wallpaper`), and that per-pixel loops use integer blending
  (`Surface::glass`). QEMU's soft-float makes per-pixel `f32` extremely slow.
- Shadow masks are generated only when a window's size changes; a resize storm
  is expected to be slower.
- Full-frame composition is a 4 MiB copy. There is no damage tracking yet.

Use `python tools/bench.py` for boot timings and `-d exec,nochain` only if you
are prepared for a very large log.

## 7. Debugging input

- `ps2: first keyboard scancode` / `first mouse byte` confirms IRQ delivery.
- QMP absolute pointer events are not translated to PS/2 relative motion;
  use `tools/pointer_test.py` (relative events) for automated mouse tests.
- The kernel key ABI delivers ASCII plus modifier bits for printable keys and
  `keys::*` codes for special keys. A key press and its release are two events;
  handlers must check `value == 1` for actions.

## 8. Kernel log ring buffer

`logging.rs` keeps the last 64 KiB in memory in addition to the serial output.
A future `dmesg` syscall will expose it to the desktop; today it is only used
for diagnostics from kernel code.

## 9. Reproducing a failure

Always record:

- the full QEMU command (`tools/run.py` prints it),
- `build/serial.log`,
- the build commit or the output of `python tools/build.py`,
- for UI issues: screenshots and pointer coordinates,
- for boot issues: `build/qemu-int.log` when `-d int,cpu_reset` was enabled.
