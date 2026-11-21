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
- The window surface is filled, title bar and traffic lights are drawn, then the
  app content is clipped to the content rect and drawn.
- Corners are cut by saving the underlying pixels and restoring them outside
  the radius, preventing app content from squaring the silhouette.
- A global surface opacity is active during the window draw, producing real
  content fade without a second buffer.

## 4. Partial compositing

The compositor sets the clip to each dirty rectangle and redraws every layer
that intersects it. `wm.draw` skips windows outside the clip, including their
app draw calls. Wallpaper is restored from the cached image first, which erases
old window, cursor and overlay pixels.

When the whole screen needs repainting (workspace slide, launcher, overview
opening), a single full-screen region is used. This is explicit and counted in
`full_redraws`.

## 5. Transparency and blur

- Alpha compositing is per primitive and supports a whole-surface multiplier.
- Glass panels (`glass`) sample the precomputed blurred wallpaper, not the live
  content behind them. This is honest "frosted wallpaper", not a real backdrop
  blur.
- The Blur switch disables that sampling and falls back to a solid translucent
  or opaque panel.
- There is no GPU blur, no mip chain and no per-frame large blur pass.

## 6. Effects quality

| Mode | Shadow blur | Shadow margin | Max dirty regions |
|---|---:|---:|---:|
| Quality | 28 | 28 | 8 |
| Balanced | 22 | 22 | 6 |
| Performance | 14 | 16 | 4 |
| Adaptive | 22 / 14 under load | 22 / 16 | 6 / 4 |

Adaptive switches to the Performance parameters when the measured average frame
time exceeds 26 ms. Changes are applied to the next frame; no restart is needed.
