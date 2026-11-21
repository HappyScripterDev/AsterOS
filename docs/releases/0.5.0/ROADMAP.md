# Aster OS 0.5.0 - Roadmap

## Completed in 0.5.0

| Area | State |
|---|---|
| Motion primitives | `aster-motion` crate: easing, tweens, springs, tracks, sequences and host unit tests |
| Frame timing | Delta/interval measurement, FPS, draw/present time, worst frame, skipped frames |
| Dirty rendering | Bounded merged dirty regions, partial wallpaper restore, partial present |
| Compositor | Layer order, clip-aware drawing, global opacity, shadow/transparency/blur switches |
| Quality modes | Quality / Balanced / Performance / Adaptive with real shadow and dirty-region changes |
| Window state machine | Opening, Normal, Minimizing, Minimized, Restoring, Closing, Maximized, Snapped |
| Window animations | Open, close, minimize-to-dock, restore, maximize/unmaximize, snap, focus shadow |
| Dock 2.0 | Bottom/left/right, size, magnification, auto-hide, indicators, minimized count |
| Workspaces | Slide transition, animated indicators, keyboard and menu navigation |
| Window overview | Captured live window previews, grid layout, keyboard/mouse selection, minimized cards |
| Quick switcher | Held-Alt overlay, app icons/titles, workspace labels, release-to-switch |
| Tabs | Text editor tab bar, create/select/close, keyboard shortcuts, live dirty markers |
| UI motion | Menu open/close fade, toast slide/fade, reduced-motion handling |
| Settings | Graphics & Motion page with real controls persisted in compact settings groups |
| Monitor | Live software-renderer FPS, frame/draw/present time, dirty pixels, redraw and animation counts |
| Documentation | Architecture, pipeline, compositor, animation, WM, dock, UI, performance, testing, accessibility |

## Partial in 0.5.0

- **Tabs:** drag-to-reorder and tab tear-out are not implemented. Tabs work by click and
  keyboard; the architecture keeps tab state separated from the document state.
- **Effects:** glass samples the composited backbuffer and applies live downsample,
  two box-blur passes, tint and noise; quality modes reduce the downsample/radius.
  There is no GPU blur.
- **Smooth scrolling:** implemented in the Text editor for Page Up/Down; the PS/2 input
  path has no wheel events, so mouse-wheel scrolling is not testable.
- **UI motion:** menus and toasts animate; individual buttons, toggles, sliders and
  dialog backdrops still use immediate highlight changes.
- **Scaling/multi-monitor:** `Display`/`DisplaySet` and a persisted `ui_scale` field
  exist, but runtime scale factors and additional monitors are not implemented.
- **Resource manager:** window/overview resources are owned by their Rust values and
  released on close, but there is no central cache with eviction or leak detector.

## Next

### 0.6.0 - Audio, hardware and device ecosystem

- HDA/AC'97 driver, audio mixer and real sound output.
- Hardware detection, ACPI/MADT, USB and Bluetooth foundations.
- Display scaling, multiple displays and GPU backend selection.
- Generic resource cache/eviction and a leak test harness.
- Mouse wheel input and full smooth scrolling for all scrollable apps.
- Tab drag/tear-out and window groups.
- Live backdrop blur only if a real GPU/accelerated path exists.
