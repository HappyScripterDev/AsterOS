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
| Per-window offscreen surfaces + compositor blit | **DONE**; each window owns a content-sized surface rendered in local coordinates and blitted at its content rect |
| Damage v2 with app-content/occlusion tracking | **PARTIAL**; window-surface validity and blit-only composition are done, app-internal damage is not |
| Live backdrop blur behind glass | **NOT DONE**; glass samples a precomputed wallpaper blur |
| Material tokens (`ultra_thin`…`sidebar`) | **NOT DONE** |
| Contrast test for glass text | **NOT DONE** |
| Stage Manager / window groups | **NOT DONE** |
| Fullscreen space (chrome hidden, restore) | **NOT DONE**; only maximize |
| Alt-Tab live previews | **DONE**; captured thumbnails with icon fallback |
| Workspace drag between desktops | **DONE**; hold a dragged window at the left/right edge for 700 ms |
| HiDPI 1.25/1.5/2.0 | **NOT DONE**; `ui_scale` is a persisted foundation only |
| VirtIO-GPU scanout/`FB_PRESENT` | **NOT DONE**; GOP direct present only |
| SIMD raster paths | **PARTIAL**; SSE2 row fill/copy for opaque rectangles and window blits, scalar alpha/text |
| Social/scratch tests `gfx_test`, `motion_test`, `stage_test`, `perf_test` | **PARTIAL**; `gfx_test.py`, `motion_test.py` and `perf_test.py` exist; `stage_test.py` has no feature to test |

The extension starts from a clean working tree and a green 0.4/0.5 regression
suite. Features are added incrementally; unimplemented items remain listed here
and in `KNOWN_LIMITATIONS.md` until they have real code and evidence.

## 6. Extension phase 1 - materials, live glass, fullscreen

Shipped after the baseline commit on `milestone/aster-0.5.0-gpu`:

- `user/asterdesk/src/material.rs`: eight light-theme material tokens
  (`ULTRA_THIN`, `THIN`, `REGULAR`, `THICK`, `CHROME`, `PANEL`, `SIDEBAR`,
  `MENU`) with tint, blur radius, downsample, saturation, noise, highlight and
  border.
- `user/asterdesk/src/glass.rs`: live backdrop blur that samples the already
  composited backbuffer, downsamples, runs two integer box-blur passes, then
  applies saturation, dither and tint. Scratch buffers are owned by `Desktop`
  and reused, so the frame loop performs no blur allocation.
- Top bar, dock, launcher, Control Centre, notification panel and toasts now use
  material tokens and live glass; the launcher's full-screen wallpaper copy is
  gone.
- `gfx::fill_rect` and `Surface::put` use integer alpha only. A full-screen
  120/255 dim dropped from ~25 ms to ~2 ms of guest time; the overview frame
  dropped from ~60 ms to ~9 ms and now lands inside the acceptance-test window.
- `wm::Window::fullscreen`: F11 / Meta+F toggles, Escape exits, chrome is hidden,
  the app receives the full screen and hidden traffic lights cannot be clicked.
  Focusing another window restores the previous rect.
- Top-bar workspace pager (click to jump, 16x16 hit targets) alongside the
  existing transient dock indicator.
- Alt-Tab cards now carry a live thumbnail captured at open time (icon fallback
  for minimized windows). Alt-Tab replaces a fading overview immediately.
- Pointer routing and focus rules for fullscreen were added to `wm.rs`.

Remaining extension gaps are unchanged: per-window surfaces and Damage v2,
Stage Manager, workspace drag, HiDPI >1.0, VirtIO-GPU scanout and `FB_PRESENT`,
and SIMD raster paths.

## 7. Extension phase 2 - per-window surfaces and window damage

- Every `wm::Window` owns a content-sized `Vec<u32>` surface. Apps draw into it
  in local coordinates `(0,0,w,h)`; the WM blits the buffer at the content rect.
  Moving a window is now a row copy and does not involve the app at all.
- Surfaces carry a validity flag and cached size. Input, size changes, a theme
  signature change and the 500 ms caret-blink tick invalidate them. Invalid
  windows re-rasterize once and contribute one dirty rectangle; valid windows
  only blit.
- `gfx::blit_window` uses `copy_nonoverlapping` rows for opaque windows and the
  regular alpha path during fades, so window opacity applies exactly once.
- `Theme::content_signature` drives palette/accent/motion invalidation without a
  per-action table.
- The heap from phase 1 now recycles window surfaces, so repeated open/close
  cycles do not grow memory.

Damage v2 is implemented at window granularity. App-internal damage (repainting
only the changed rows of a text view) remains a documented gap.

## 8. Extension phase 3 - SIMD rows and desktop drag

- `gfx` now has SSE2 row helpers: `fill_row` for opaque rectangles and `clear`,
  and `copy_row` for the opaque `blit_window` path. The 16-byte loop handles the
  body and a scalar tail closes each row. Alpha blends, text and rounded paths
  remain scalar by design.
- Dragging a window against the left/right edge and holding it for 700 ms drops
  it on the adjacent desktop and follows it there. The dwell waits 300 ms before
  hiding the snap preview, so a quick drag still snaps.
- The window manager now really has four desktops; the pager previously drew
  three marks even though the specification called for four.
- New `tools/gfx_test.py` covers moved-window surface restoration, fullscreen
  dock hiding/restore, pager jumps and the edge-dwell desktop drop.

## 9. Remaining gaps after the extension

The following brief items are deliberately not claimed:

- **Stage Manager / window groups.** No stage strip, grouped windows or stage
  transitions exist. `stage_test.py` is therefore absent; adding a test without
  the feature would be a placeholder.
- **HiDPI 1.25/1.5/2.0.** `ui_scale` is persisted and surfaced in Settings but
  the compositor still lays out and rasterizes at 1.0. Implementing this needs a
  full logical-pixel transform and a scaled text/raster path; it is not faked
  with a larger font.
- **VirtIO-GPU scanout and `FB_PRESENT(47)`.** No virtio-gpu driver, no resource
  creation, no scanout buffer and no new syscall were added. The GOP framebuffer
  is the only present path, so GPU rasterization is never claimed.
- **App-internal damage.** Window surfaces are cached and invalidated at window
  granularity; apps do not expose changed rows yet.
- **Multi-monitor / runtime mode setting.** Display structures remain a
  foundation only.

Delivered instead, with tests: live glass and materials, material/fullscreen
UI, fullscreen windows, top-bar pager, live Alt-Tab thumbnails, per-window
surfaces, window-level damage, a coalescing heap, SSE2 opaque rows, desktop drag
and the four-desktop model. `tools/gfx_test.py`, `tools/motion_test.py` and
`tools/perf_test.py` cover the new engine paths; the existing 0.4/0.5 suite
remains green.
