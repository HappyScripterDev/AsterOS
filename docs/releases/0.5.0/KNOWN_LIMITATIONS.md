# Aster OS 0.5.0 - Known Limitations

This is the honest list of what 0.5.0 does not do or does with weaker guarantees.

## Rendering and effects

- **Software only.** No GPU driver, no GPU composition, no shaders and no hardware
  acceleration. Monitor shows "software renderer"; no GPU percentage is shown.
- **No vsync, triple buffering or refresh-rate detection.** Frame pacing is a
  millisecond budget. QEMU GOP exposes no vsync signal.
- **Blur is wallpaper blur.** Glass panels sample a precomputed blurred wallpaper,
  not the live pixels behind the panel. Turning Blur off uses a solid fallback.
- **No live backdrop blur, depth-of-field or large blur passes.**
- **Dirty regions are compositor rectangles, not arbitrary regions.** They are
  merged and capped; a busy frame above 62% area becomes a full redraw.
- **Window shadows are cached masks.** Quality changes require a mask rebuild;
  during a resize the mask is regenerated for the new size.
- **No damage tracking inside an app.** A window is repainted as a whole (clipped
  to the dirty region), not by app-internal dirty rectangles.
- **Overview captures previews at open time.** They are real pixels from the
  backbuffer, but minimized windows show an app icon rather than a live preview.

## Motion and UI

- Window transitions are short and do not expose pause/reverse in the UI, although
  the primitives support them.
- Buttons, toggles, checkboxes, sliders and dialog backdrops still change state
  immediately; only menus, toasts, windows, dock, workspace, overview and switcher
  animate.
- Drag-to-reorder tabs, tab tear-out, window groups and reorder animations are not
  implemented.
- Smooth scrolling is Text Page Up/Down only. The PS/2 input path has no wheel
  events, so mouse-wheel scrolling is not available.
- The Overview shows the current window set at open; newly opened windows while
  the overview is visible are not added until it is reopened.
- The quick switcher uses app icons and titles; it does not render live thumbnails.
- No live animation preview outside Settings; the preview card is a small moving
  rectangle, not a full desktop demo.

## Display, scaling and hardware

- One display at 1280×800, scale factor 1.0. `Display`/`DisplaySet` and `ui_scale`
  are foundations, not implemented multi-monitor/HiDPI support.
- No runtime resolution change, rotation, monitor arrangement or per-monitor
  workspace.
- No accessibility tree, screen reader, high-contrast mode or keyboard focus ring
  on every component.
- No GPU backend interface implementation; no backend will be claimed until real
  hardware acceleration exists.

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
