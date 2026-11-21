# Aster OS 0.5.0 - Compositor

## 1. Responsibilities

The desktop compositor owns layer order, dirty-region repaint, window clipping,
window opacity and effect quality. Applications never touch the framebuffer;
they draw through `Surface` into a rect supplied by the window manager.

## 2. Layer ordering

| Order | Layer | Notes |
|---:|---|---|
| 1 | Wallpaper | Cached generated image, copied only in dirty regions |
| 2 | Windows | Sorted by z; current workspace only; shadows and rounded clip |
| 3 | Dock | Bottom/left/right; glass or solid fallback |
| 4 | Top bar | Clock, app name, menus, status icons, tooltip |
| 5 | Panels/menus | Control Centre, notification centre, application menus |
| 6 | Toasts | Up to three unread, with slide/fade |
| 7 | Workspace indicators | Visible during/after a desktop change |
| 8 | Overview | Full-screen dim layer plus window cards |
| 9 | Quick switcher | Dim layer plus centred window strip |
| 10 | Cursor | Always drawn last so it is never delayed by animations |
| 11 | Brightness | Display-wide dim overlay |

## 3. Window composition

- Window targets are integer rectangles. Transitions compute an interpolated
  rectangle from `from`/`to` and an opacity from 1→0 or 0→1.
- Shadows are generated masks cached per window size. Quality mode selects
  blur radius and margin; shadows can be disabled.
- Every window owns a content-sized `Vec<u32>` surface. The app draws into it in
  local coordinates `(0,0,w,h)`; the compositor blits the buffer at the content
  rect. Moving a window is blit-only and does not re-render the app.
- A surface is valid until its size changes, the app receives input, the theme
  signature changes, or the 500 ms blink tick fires. Only invalid surfaces are
  re-rasterized; valid ones are blitted from cache.
- The window frame is filled, title bar and traffic lights are drawn, then the
  cached content is blitted. Corners are cut by saving the underlying pixels and
  restoring them outside the radius.
- A global surface opacity is active during the window draw, so both chrome and
  cached content fade with the same multiplier.

## 4. Partial compositing

The compositor sets the clip to each dirty rectangle and redraws every layer
that intersects it. `wm.draw` skips windows outside the clip, including their
blits. Wallpaper is restored from the cached image first, which erases old
window, cursor and overlay pixels.

Window damage is tracked at surface granularity: a window's content buffer has a
valid flag and a cached size. Invalid windows are redrawn once into their buffer
and contribute one dirty rect; valid windows only blit. Input, a size change, a
theme-signature change or the 500 ms caret blink mark a surface invalid. There is
no app-internal damage API yet, so any invalidation re-rasterizes the whole window
content.

When the whole screen needs repainting (workspace slide, launcher, overview
opening), a single full-screen region is used. This is explicit and counted in
`full_redraws`.

## 5. Transparency and blur

- Alpha compositing is per primitive and supports a whole-surface multiplier.
- Glass panels (`glass_live`) sample the composited backbuffer, downsample it,
  run two box-blur passes and tint the result. This is a real backdrop blur over
  wallpaper and window content; see DESIGN_SYSTEM.md 6.3.
- The Blur switch disables sampling and falls back to the material tint (or the
  opaque surface when Reduce Transparency is on).
- There is no GPU blur, no mip chain and no full-screen blur pass. Blur cost is
  proportional to the panel area and quality mode.

## 6. Effects quality

| Mode | Shadow blur | Shadow margin | Max dirty regions |
|---|---:|---:|---:|
| Quality | 28 | 28 | 8 |
| Balanced | 22 | 22 | 6 |
| Performance | 14 | 16 | 4 |
| Adaptive | 22 / 14 under load | 22 / 16 | 6 / 4 |

Adaptive switches to the Performance parameters when the measured average frame
time exceeds 26 ms. Changes are applied to the next frame; no restart is needed.
