# Aster OS 0.5.0 - Test Report

Date: 2026-11-21
Branch: `milestone/aster-0.5.0`
Reference machine: QEMU q35, 1 vCPU, 512 MiB, OVMF, 1280×800, software renderer.

## 1. Final suite

Every line below was run against the final tree and exited 0.

| Test | Result |
|---|---|
| `cargo test -p aster-motion` (inside `tools/test.py`) | PASS, 8 tests |
| `python tools/test.py` | PASS: host tests, build, boot smoke, no faults |
| `python tools/ui_test.py` | PASS |
| `python tools/graphics_test.py` | PASS: pixels and persistence |
| `python tools/app_probe.py` | PASS |
| `python tools/pointer_test.py` | PASS |
| `python tools/wm_probe.py` | PASS |
| `python tools/files_probe.py` | PASS |
| `python tools/fs_probe.py` | PASS |
| `python tools/persistence_test.py` | PASS: write/reboot/read and settings format |
| `python tools/storage_probe.py` | PASS |
| `python tools/isolation_test.py` | PASS |
| `python tools/settings_test.py` | PASS: corrupt/invalid/valid settings |
| `python tools/service_test.py` | PASS |
| `python tools/final_ui_probe.py`, `status_probe.py`, `shell_probe.py`, `about_probe.py`, `accent_probe.py` | PASS |
| `python tools/net_test.py` | PASS: DHCP, ICMP, TCP echo/bulk, UDP, DNS |
| `python tools/network_ui_probe.py` | PASS |

## 2. Graphics acceptance details

`tools/graphics_test.py` verifies:

- Files minimize leaves no residue in the old window rectangle (pixel compare
  against the pre-window wallpaper);
- clicking the Files dock icon restores the window (white content pixel);
- F3 Window Overview dims the desktop and draws real preview cards;
- held-Alt quick switcher shows a light panel over a dimmed desktop;
- Ctrl+Right leaves desktop 1 window content behind and Ctrl+Left restores it;
- the Text editor creates a second tab and selects the first tab (serial
  evidence `text: tab 2 of 2` / `text: tab 1 of 2`);
- Settings navigates to Graphics & Motion (`settings: section 9`);
- turning Window animations off writes `motion=0,1,1,1,1` to `/SETTINGS.CNF`
  on the data disk;
- after rapid close/workspace/overview stress, a fresh Terminal still answers
  `uname`;
- no kernel or desktop panic appears in the serial log.

## 3. Measured results

- boot to first compositor frame: 3.612 s;
- frame 120: 16.8 ms average, 59.6 fps;
- first full frame: 1,024,000 dirty pixels, 28 ms draw, 14 ms present;
- welcome-toast frames: ~37,024 dirty pixels, 7–8 ms draw, 0–1 ms present;
- after toast expiry: 120/120 frames skipped with zero draw/present work;
- free RAM after boot: 442 MiB;
- network tests still pass unchanged (DHCP/ICMP/TCP/UDP/DNS).

## 4. Screenshots

`build/graphics/` contains:

`00-baseline.ppm`, `01-windows.ppm`, `02-minimized.ppm`, `03-restored.ppm`,
`04-overview.ppm`, `05-switcher.ppm`, `06-desktop2.ppm`, `07-desktop1.ppm`,
`08-tabs.ppm`, `09-graphics-settings.ppm`, `09b-window-animations-off.ppm`,
`10-after-stress.ppm`.

Documentation screenshots:
`docs/screenshots/graphics-overview.png`, `docs/screenshots/graphics-settings.png`.

## 5. Not tested

- Physical hardware or a GPU.
- Forced low-FPS/lag injection; animations use wall time but were not stress-run
  at 5–10 fps.
- Drag-to-reorder/tab tear-out (not implemented).
- Live backdrop blur (not implemented).
- Multi-monitor and runtime scale factors (not implemented).
- Long-running soak/leak detection.
- Automated pixel comparison of menu/toast fades; these are covered visually.
