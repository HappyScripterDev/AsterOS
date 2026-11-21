# Aster OS 0.5.0 - Rendering Pipeline

## 1. Frame order

Each desktop loop iteration:

1. Read `TIME_MS`; compute the interval since the previous loop.
2. Poll up to 64 input events and update mouse/keyboard state.
3. Update clock/network cache.
4. Update dock auto-hide and publish dock anchors.
5. Tick applications and window transitions.
6. Tick the settings service supervisor.
7. Collect dirty rectangles (input, transitions, overlays, clock, cursor, live
   app refresh).
8. If no dirty work exists, increment the skipped counter and sleep to the frame
   budget. Nothing is rendered or presented.
9. Otherwise snapshot the dirty regions, paint each region, present each region,
   record statistics and clear the dirty set.
10. Sleep only for the remaining configured budget.

This is a coalesced dirty renderer, not a retained command-list renderer. Each
dirty region is repainted from the wallpaper up through every layer that
intersects it.

## 2. Dirty-region rules

`render::DirtyRegions` keeps at most eight rectangles (fewer in
Quality/Performance modes) and merges rectangles that intersect or are within
6 px. When the region count exceeds the limit, the two smallest regions are
coalesced with the new one. If the dirty area reaches 62% of the screen, the
frame becomes a full redraw.

Sources:

- window transitions (union of current, from and to bounds plus shadow margin);
- window drag/resize (old and new bounds);
- focus shadow animation;
- cursor old/new position;
- dock hover, dock auto-hide and dock geometry changes;
- clock/network menu-bar changes;
- open/close of menu, Control Centre, notification panel, launcher;
- active toasts;
- workspace switch (full screen);
- a 500 ms refresh of visible windows for live apps such as Monitor and Network;
- `needs_full` after wallpaper, accent, brightness, dock or window structural
  changes.

## 3. Painting a dirty region

1. `surface.set_clip(region)`.
2. Copy only the wallpaper rows intersecting the region from the cached wallpaper
   buffer (raw memory copy, no blending).
3. Draw the window layer with `wm.draw(..., clip)`. Windows outside the clip are
   skipped; app content is clipped to the intersection.
4. Draw dock, top bar, panels, menus and toasts; each primitive respects the clip.
5. Draw workspace indicators, overview/switcher, then cursor.
6. Apply the brightness overlay to the region.
7. `present_region` copies only that rectangle to the GOP framebuffer.
8. Reset clip and opacity to full.

Full redraws copy the whole wallpaper and draw one full-screen region.

## 4. Buffering

- One persistent backbuffer (`gfx::Surface`) sized to the display.
- The framebuffer is the front buffer; presentation is a row copy (or channel
  conversion for non-XRGB GOP formats).
- No triple buffering or vsync exists. Pacing is a millisecond frame budget;
  QEMU's framebuffer has no vsync signal exposed to the kernel.
- Partial present is real: an idle clock tick pushes roughly 33 KiB rather than
  4 MiB.

## 5. Clipping and alpha

- `Surface::clip` is an integer rectangle intersected by every primitive.
- `Surface::opacity` multiplies primitive alpha, including opaque fills, so a
  window can fade as a whole without an offscreen buffer.
- The rounded-corner implementation saves and restores the underlying corner
  pixels; this remains correct with partial regions because saved pixels come
  from the freshly restored wallpaper/window layer.
- Transparent window content is composited over whatever the region contains
  after wallpaper and lower windows.

## 6. Frame pacing and timing

- `FramePacer` supports 30, 60, 120 fps and Free (no sleep).
- Animation time always comes from `TIME_MS`, never frame counts, so skipped or
  late frames do not stretch or lose animations.
- Frame stats include measured interval, input time, draw time, present time,
  average, worst, dirty pixels, full redraws and skipped frames.
