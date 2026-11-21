# Aster OS 0.5.0 - Baseline Audit

Date: 2026-11-21
Branch: `milestone/aster-0.5.0`
Baseline tag: `v0.4.0-baseline` (commit `16f21dc`)
Previous release: `v0.4.0`

This document records the verified rendering state at the start of 0.5.0.

## 1. Verification

| Check | Result |
|---|---|
| `python tools/build.py` | PASS: bootloader, kernel, desktop, 512 MiB image |
| `python tools/test.py` | PASS: host tests, build, boot smoke |
| `python tools/ui_test.py`, `app_probe.py`, `pointer_test.py`, `wm_probe.py` | PASS |
| `python tools/persistence_test.py`, `storage_probe.py`, `files_probe.py`, `fs_probe.py` | PASS |
| `python tools/settings_test.py`, `service_test.py`, `isolation_test.py` | PASS |
| `python tools/net_test.py`, `network_ui_probe.py` | PASS |
| Benchmark | first frame 3.47 s, idle 22.6 ms, 442 MiB free |

## 2. Rendering state

- **Backend:** software only, one 1280x800 32-bit XRGB backbuffer (`gfx::Surface`), presented to the UEFI GOP framebuffer. No GPU driver or 3D acceleration exists.
- **Frame loop:** every loop polls input, ticks the WM, then copies the entire wallpaper into the backbuffer and redraws the whole screen: window chrome, applications, dock, menu bar, overlays and cursor. The full backbuffer is then converted/copied to the framebuffer.
- **Timing:** a fixed `sleep_ms(16 - elapsed)` pacing; no frame budget configuration, no dirty regions, no skipped idle frames, no render statistics exposed to userspace.
- **Effects:** analytic rounded rectangles/circles, per-primitive alpha, cached window shadows, precomputed blurred wallpaper (`box_blur`) used by dock/menu/launcher glass panels, soft shadows. The blur is a wallpaper blur, not a live backdrop blur.
- **Windowing:** one `Window` record per app with integer bounds, z-order, drag/resize/snap interaction and timestamp-based open/close/minimize effects. There is no explicit window state machine. Minimize always targets the dock centre, not the app icon.
- **Dock:** fixed bottom position, fixed 50 px icons / 62 px slots, cursor-proximity magnification, running dots. No dock settings, auto-hide or position changes.
- **Workspaces:** three virtual desktops with instant switching; no transition and no workspace indicator.
- **UI motion:** window open slide/scale, close scale and minimize scale, timestamp-driven. Menus, Control Centre, notifications, buttons, toggles and sliders change instantly. Toasts are drawn only when `draw_toasts` is called from the click path, not as part of every frame.
- **Multitasking:** no window overview, no Alt+Tab overlay, no tabs.
- **Settings:** Appearance/Display/Storage/Network etc. Motion settings are limited to one animation-speed value and a reduce-motion switch. There is no Graphics & Motion page.

## 3. Code size at baseline

| File | Size |
|---|---|
| `user/asterdesk/src/apps.rs` | 161,127 bytes |
| `user/asterdesk/src/desktop.rs` | 66,727 bytes |
| `user/asterdesk/src/gfx.rs` | 38,119 bytes |
| `user/asterdesk/src/wm.rs` | 27,409 bytes |
| `user/asterdesk/src/ui.rs` | 30,793 bytes |
| `user/asterdesk/src/theme.rs` | 14,518 bytes |

## 4. Constraints for 0.5.0

1. Keep the software renderer; no fake GPU acceleration.
2. Preserve storage, networking, services and all 0.4.0 tests.
3. Add time-based motion, never input-blocking sleeps in animation code.
4. Every settings control must change real behaviour and persist through the existing `version=1` settings service.
5. Keep the 1280x800 QEMU target working and document scaling/multi-monitor as foundations only.

## 5. Extension audit (2026-11-21, `milestone/aster-0.5.0-gpu`)

The repository already contains a **committed** 0.5.0 graphics/motion baseline
(tag `v0.5.0`, commit `f621a77`). It is preserved as the baseline for this
extension. Audit against the extended engine brief:

| Requirement | Actual state |
|---|---|
| Dirty-region compositor, partial present, idle skip | DONE in `render.rs` and `desktop.rs` |
| `aster-motion` easing/tween/spring/tracks/tests | DONE, 8 host tests |
| Window state machine and animated transitions | DONE |
| Dock 2.0, Overview, Alt-Tab, workspace slide, Text tabs | DONE |
| Per-window offscreen surfaces + compositor blit | **NOT DONE**; windows draw directly into the desktop surface |
| Damage v2 with app-content/occlusion tracking | **PARTIAL**; compositor-level rectangles only |
| Live backdrop blur behind glass | **NOT DONE**; glass samples a precomputed wallpaper blur |
| Material tokens (`ultra_thin`…`sidebar`) | **NOT DONE** |
| Contrast test for glass text | **NOT DONE** |
| Stage Manager / window groups | **NOT DONE** |
| Fullscreen space (chrome hidden, restore) | **NOT DONE**; only maximize |
| Alt-Tab live previews | **PARTIAL**; icons/titles only |
| Workspace drag between desktops | **NOT DONE** |
| HiDPI 1.25/1.5/2.0 | **NOT DONE**; `ui_scale` is a persisted foundation only |
| VirtIO-GPU scanout/`FB_PRESENT` | **NOT DONE**; GOP direct present only |
| SIMD raster paths | **NOT DONE** |
| Social/scratch tests `gfx_test`, `motion_test`, `stage_test`, `perf_test` | **PARTIAL**; `graphics_test.py` and motion unit tests exist |

The extension starts from a clean working tree and a green 0.4/0.5 regression
suite. Features are added incrementally; unimplemented items remain listed here
and in `KNOWN_LIMITATIONS.md` until they have real code and evidence.
