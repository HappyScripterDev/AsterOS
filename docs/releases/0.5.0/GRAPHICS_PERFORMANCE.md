# Aster OS 0.5.0 - Graphics Performance

Reference: QEMU q35, 1 vCPU, 512 MiB, OVMF, 1280×800, TCG, software renderer.

## 1. Before / after

| Metric | 0.4.0 | 0.5.0 |
|---|---:|---:|
| Frame strategy | full 1280×800 repaint every frame | dirty regions; idle frames skipped |
| Backbuffer present | full 4 MiB every frame | only dirty rectangles |
| Idle rendering | always ~22.6 ms work/frame | 0 draw/present after toast expiry; loop sleeps |
| Frame budget | fixed 16 ms sleep | 30/60/120/Free, delta-based |
| Window animations | timestamps, no opacity | state machine, geometry + real opacity |
| Overview/switcher | absent | present (full-screen while open) |
| Boot to first frame | 3.47 s (0.4 benchmark) | 3.61 s (this benchmark; TCG variance) |
| Free RAM after boot | 442 MiB | 442 MiB |

The 0.4.0 number and the 0.5.0 number come from separate TCG runs. Several
hundred milliseconds of run-to-run drift is normal on this host.

## 2. Measured 0.5.0 results

From `tools/bench.py` and the idle probe:

- first compositor frame: **3.612 s** wall;
- frame 120: **16.8 ms average, 59.6 fps**;
- first full frame: 1,024,000 dirty pixels, 28 ms draw, 14 ms present;
- during the 6 s welcome toast: ~37,024 dirty pixels/frame, 7-8 ms draw,
  ~0-1 ms present;
- after toast expiry: **120 of 120 frames skipped** in consecutive intervals;
  draw and present are 0, the loop still polls input;
- idle loop cadence is ~20 ms (about 50 Hz) because the kernel sleep and timer
  granularity are milliseconds; skipped frames do no rendering work.

## 3. Interaction costs

- Window open/minimize/close: dirty region is the union of current/from/to plus
  shadow; no full redraw except the cleanup frame after minimize/close.
- Window drag/resize: old and new window bounds are dirty; apps redraw only
  inside the clip.
- Overview opening: first frames are full-screen, measured around **90 ms draw**
  in one probe run, then the cached card layout skips full repaints after the
  420 ms opening window.
- Switcher: small centre panel, full-screen dim backdrop while held; cheap draw.
- Workspace slide: full-screen for 280 ms because both desktops are composited
  at a horizontal offset.
- Dock hover/auto-hide: dock rectangle only.

## 4. Quality modes

| Mode | Shadow blur | Max dirty regions | Effect |
|---|---:|---:|---|
| Quality | 28 | 8 | sharpest shadows, finest region tracking |
| Balanced | 22 | 6 | default |
| Performance | 14 | 4 | smaller shadow masks, more merging |
| Adaptive | 22 or 14 | 6 or 4 | switches to Performance when average frame > 26 ms |

Shadow generation is cached per window size. Changing size invalidates only
that window's mask. Quality changes are immediate and visible; no FPS claim is
made for a mode that was not separately benchmarked.

## 5. Memory

- Backbuffer: 1280×800×4 = 4 MiB.
- Wallpaper + blurred wallpaper: ~8 MiB combined.
- Window shadows: allocated per size, released with the window.
- Overview: one thumbnail per window, released when the overview closes.
- Dock shadow: one mask, regenerated only when dock size/position changes.
- Free RAM after boot remains 442 MiB, indicating no large permanent allocation
  was added by the engine.

## 6. Not measured / limits

- No physical GPU (no GPU backend exists).
- No sustained multi-window FPS benchmark at 60 Hz; QEMU TCG timing is noisy.
- Overview opening cost scales with the number and size of captured windows.
- No leak detector; resource release is by Rust ownership and close paths.
- No CPU-utilization sampling of the idle loop beyond skipped frames and sleep
  cadence.
