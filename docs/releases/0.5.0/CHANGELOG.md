# Aster OS 0.5.0 - Changelog

Release: **0.5.0 graphics and motion**
Date: 2026-11-21

## Graphics engine

- New `shared/aster-motion` crate with easing curves, tweens, critically damped
  springs, named motion tracks, cancellation and sequences. Eight host unit
  tests run with `cargo test -p aster-motion`.
- New `user/asterdesk/src/render.rs`:
  - bounded dirty-region merging (8 regions, quality-dependent cap);
  - full-screen fallback above 62% dirty area or excessive region count;
  - measured frame, input, draw, present, FPS, worst-frame and skipped-frame stats;
  - Quality/Balanced/Performance/Adaptive modes that change shadow blur/margin and
    dirty-region limits; Adaptive switches to Performance above 26 ms average;
  - display/multi-monitor foundation (`Display`, `DisplaySet`, persisted UI scale);
  - lock-free `EngineSnapshot` for Monitor.
- `gfx::Surface` gained global opacity (real window fading), partial
  `present_region`, opaque `copy_region` for previews, and safe scaling bounds.
- The desktop now restores only dirty wallpaper regions, composites only the
  layers intersecting the dirty clip, and presents only dirty rectangles. Idle
  frames are skipped entirely when nothing changed.

## Window manager

- Explicit `WindowState` machine and `Transition` records:
  `Opening`, `Normal`, `Minimizing`, `Minimized`, `Restoring`, `Closing`,
  `Maximized`, `Snapped`.
- Time-based open, close, minimize-to-dock-icon, restore, maximize/unmaximize,
  snap and focus-shadow transitions.
- Real opacity fading through per-surface alpha, including app content.
- Per-app dock anchors; the minimize target follows dock position, size and
  magnification.
- Focus change animates shadow strength; hit-testing always uses the target
  rectangle so input is never blocked by an animation.
- Closed/minimized windows repaint their previous area; removed windows are
  cleaned up without leftovers.

## Dock 2.0

- Bottom, left and right positions with real geometry and work-area reservation.
- Dock size, icon scale, magnification, auto-hide, running indicators and
  minimized-count indicators.
- Smooth auto-hide driven by cursor proximity; no animation while reduced motion
  is active.
- All values persist through the settings service and apply immediately.

## Workspaces and multitasking

- Horizontal workspace slide transition with animated desktop indicators.
- Ctrl+Left/Right and Window menu use the same transition path.
- Window overview (F3 / Ctrl+Up): real captured previews of each window, grid
  layout, selected outline, app icon, title, workspace and minimized state.
  Mouse and keyboard selection; Enter focuses and restores.
- Held-Alt quick switcher: app icons, titles, workspace, minimized markers,
  selection highlight, release-Alt commit and Esc cancel.

## Tabs

- Text editor tab bar with create, select, close and dirty markers.
- `Ctrl+T`, `Ctrl+PageUp/PageDown`, `Ctrl+1..8`; click and close buttons.
- Tabs preserve independent text, cursor, scroll and dirty state.
- Opening a file from Files adds/selects a tab instead of discarding the
  current document.

## UI motion and effects

- Menu open fade and close fade.
- Toast slide-in/fade-out with real time-based opacity and dismissal.
- Reduced-motion handling for windows, dock, workspace, overview, switcher,
  menus, toasts and Text scrolling.
- Real effect switches for window/dock shadows, transparency and the
  precomputed wallpaper blur used by glass panels.
- Settings > Graphics & Motion: performance mode, animation speed, dock size,
  magnification, frame limit, dock position, window/dock/workspace/UI animation
  switches, smooth scrolling, shadows, transparency, blur, indicators and
  auto-hide. A live motion preview uses the same theme values.

## Monitor

- Graphics card showing measured FPS plus a real software-renderer line:
  frame time, draw time, present time, dirty regions/pixels, full redraws,
  animation count and active windows. No GPU metrics are shown because no GPU
  backend exists.

## Versioning and tests

- Version strings updated to 0.5.0.
- `tools/graphics_test.py` drives QEMU and checks real pixels for minimize/
  restore, overview dimming, switcher visibility, workspace switching, tabs,
  Settings section 9 and persisted graphics settings.
- `tools/test.py --ui` now runs the graphics test; the host suite runs the
  `aster-motion` unit tests.
- `tools/persistence_test.py` and `tools/settings_test.py` updated to the new
  compact settings groups.

## Extension: graphics engine phase 1 (2026-11-21)

- Added material tokens and live glass backdrop blur over the composited
  backbuffer, with reusable scratch buffers and quality-scaled radius/downsample.
- Optimised solid and translucent rectangles to integer alpha with row-wise
  blends; a 1280x800 dim now costs ~2 ms of guest time instead of ~25 ms.
- Replaced the desktop bump allocator with a first-fit heap that splits blocks
  and coalesces neighbours on free. Preview and string buffers now return to the
  heap, so long sessions no longer exhaust the 48 MiB arena.
- Added fullscreen windows (F11 / Meta+F, Escape to exit) that hide the top bar
  and dock and route the whole surface to the app.
- Added a clickable top-bar workspace pager and live Alt-Tab window thumbnails.
- Documented the shipped glass recipe, fullscreen behaviour and shortcuts in the
  design system, desktop spec and interaction spec.
