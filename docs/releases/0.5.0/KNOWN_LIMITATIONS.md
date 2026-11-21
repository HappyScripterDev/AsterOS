# Aster OS 0.5.0 - Known Limitations

This is the honest list of what 0.5.0 does not do or does with weaker guarantees.

## Rendering and effects

- **Software only.** No GPU driver, no GPU composition, no shaders and no hardware
  acceleration. Monitor shows "software renderer"; no GPU percentage is shown.
- **No vsync, triple buffering or refresh-rate detection.** Frame pacing is a
  millisecond budget. QEMU GOP exposes no vsync signal.
- **Glass blur is live but software.** Panels sample the composited backbuffer,
  downsample it and run two integer box-blur passes before tinting. There is no
  gaussian blur, no GPU blur and no blur of the full screen; quality modes reduce
  the downsample/radius. Large panels cost more than the wallpaper-only fallback.
- **SIMD covers opaque rows only.** SSE2 is used for solid fills and opaque window
  blits; alpha blends, rounded rectangles, text and blur remain scalar.
- **Dirty regions are compositor rectangles, not arbitrary regions.** They are
  merged and capped; a busy frame above 62% area becomes a full redraw.
- **Window shadows are cached masks.** Quality changes require a mask rebuild;
  during a resize the mask is regenerated for the new size.
- **Window content damage is whole-window.** Each window owns a content-sized
  offscreen surface and is re-rasterized only when invalid (input, resize, theme
  change or the 500 ms caret tick). There is no app-internal damage API yet, so
  a single changed character repaints the whole window content once.
- **Overview captures previews at open time.** They are real pixels from the
  backbuffer, but minimized windows show an app icon rather than a live preview.
- **Fullscreen is one window at a time.** Focusing another window restores the
  fullscreen window to its previous rectangle; there is no separate fullscreen
  space or multi-monitor fullscreen.

## Motion and UI

- Window transitions are short and do not expose pause/reverse in the UI, although
  the primitives support them.
- Buttons, toggles, checkboxes, sliders and dialog backdrops still change state
  immediately; only menus, toasts, windows, dock, workspace, overview and switcher
  animate.
- Drag-to-reorder tabs, tab tear-out, Stage Manager, window groups and reorder
  animations are not implemented.
- `perf_test.py` is a smoke test over the compositor's own frame log, not a
  benchmark; QEMU TCG wall time is not representative of hardware.
- Smooth scrolling is Text Page Up/Down only. The PS/2 input path has no wheel
  events, so mouse-wheel scrolling is not available.
- The Overview shows the current window set at open; newly opened windows while
  the overview is visible are not added until it is reopened.
- The quick switcher renders a live thumbnail captured at open; thumbnails do not
  refresh while Alt is held.
- The desktop drop gesture has no ribbon or target preview; the window simply
  follows the desktop switch after the 700 ms dwell.
- No live animation preview outside Settings; the preview card is a small moving
  rectangle, not a full desktop demo.

## Display, scaling and hardware

- One display at 1280×800. HUD and UI scale 1.25/1.5/2.0 are **not implemented**:
  `ui_scale` is persisted and exposed, but the compositor still lays out at 1.0.
  `Display`/`DisplaySet` remain a multi-monitor foundation only.
- No runtime resolution change, rotation, monitor arrangement or per-monitor
  workspace.
- No accessibility tree, screen reader, high-contrast mode or keyboard focus ring
  on every component.
- No VirtIO-GPU scanout and no `FB_PRESENT(47)`. The compositor presents directly
  to the GOP framebuffer; there is no GPU backend, and no acceleration is claimed.

## Resource management and stability

- No central graphics resource cache, eviction policy or texture manager.
- No automated leak detector. Resources are freed by Rust ownership/close paths.
- No long-running soak test or forced low-FPS stress beyond `graphics_test.py`.
- Animation state is per-window and UI; a future multi-window app model will need
  a shared resource/animation registry.
- The desktop remains a single ring-3 process; built-in apps still share its
  address space and a panic takes down the shell.

## Preserved 0.4.0 limitations

- FAT32 uses 8.3 names, no journal/permissions.
- Network stack is IPv4-only, polled, no inbound TCP/IPv6/TLS.
- No KILL, per-process quotas, ASLR/SMEP/SMAP/UMIP or SMP.
- No audio, USB, GPU or installer/update service.
- Version 0.5.0 still reports Aster ABI 1; no new syscalls were added.
