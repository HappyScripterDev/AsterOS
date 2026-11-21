# Aster OS 0.5.0 - Graphics Engine Architecture

## 1. Overview

Aster OS remains a software-rendered desktop. The 0.5.0 engine separates concerns
without replacing working primitives:

```
                    +---------------------------+
 desktop state  ->  | Desktop (shell/layout)    |
                    |  dirty collection         |
                    |  frame pacing             |
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    | WindowManager             |
                    |  state machine            |
                    |  transitions, hit tests   |
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    | gfx::Surface              |
                    |  clip, opacity, primitives|
                    |  text/icons/shadows       |
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    | present_region            |
                    |  XRGB -> GOP framebuffer  |
                    +---------------------------+
```

`render.rs` owns timing, dirty regions, quality modes, display description and
telemetry. `aster-motion` owns time-based animation primitives. `wm.rs` owns
window state and transitions. `gfx.rs` remains the pixel/primitive layer.

## 2. Modules

| Module | Responsibility |
|---|---|
| `shared/aster-motion` | Easing, `Tween`, `Spring`, `MotionSystem`, `Sequence`, host tests |
| `user/asterdesk/src/render.rs` | Dirty regions, frame stats, frame pacer, quality, displays, telemetry |
| `user/asterdesk/src/gfx.rs` | Surface, clipping, alpha, primitives, text, icons, images, shadows |
| `user/asterdesk/src/wm.rs` | Window records, state machine, transitions, input, z-order |
| `user/asterdesk/src/desktop.rs` | Shell layers, dock, menu bar, workspace, overview, switcher, effects |
| `user/asterdesk/src/ui.rs` | Reusable components (buttons, cards, toggles, sliders, icons) |
| `user/asterdesk/src/apps.rs` | Applications, tabs, Settings/Monitor graphics UI |
| `user/asterdesk/src/theme.rs` | Colors, accent, motion/effects/dock configuration |

## 3. Backends and hardware

- **Active backend:** software rasterizer into a 32-bit backbuffer, converted to
  the GOP framebuffer. There is no GPU command stream, shader compiler or
  hardware blitter.
- **No fake acceleration:** the Monitor says "software renderer"; there are no
  GPU utilization numbers.
- **Prepared boundaries:** `Display`/`DisplaySet` describe physical displays and
  a scale factor; `QualityMode` and `EngineSnapshot` are backend-neutral. A
  future GPU backend must implement the same surface/present contract before it
  can be selected. No unused GPU trait is claimed as implemented.
- **Multi-monitor:** `DisplaySet` supports up to four entries and currently
  contains exactly the one display reported by the boot framebuffer. Window
  placement still uses one screen rectangle.

## 4. Layer order

Back to front:

1. wallpaper
2. windows (sorted by z-order, current workspace)
3. dock
4. top bar
5. Control Centre / notification panel / menus
6. toasts
7. workspace indicators
8. overview
9. quick switcher
10. cursor
11. brightness overlay

Every layer is clipped to the current dirty rectangle during partial rendering.

## 5. Resource ownership

- Window shadows are cached per size and freed with the window.
- Overview thumbnails are captured once when the overview opens and freed when
  it closes or a window is selected.
- Motion tracks are stored in `MotionSystem` and collected after completion;
  the system allocates on `start`, not per frame.
- Application state remains owned by `App` values inside windows.
- There is no central texture cache or eviction policy yet; images/icons are
  static assets or generated per use.

## 6. Files and measurement

- Frame logs: `desktop: frame N ... ms avg (... fps, input, draw, present, dirty,
  full, skipped)`.
- Monitor reads `render::snapshot()`: FPS, frame/draw/present ms, dirty regions
  and pixels, full redraws, animations and active windows.
- No synthetic FPS or GPU values are presented.
