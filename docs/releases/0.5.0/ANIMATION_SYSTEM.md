# Aster OS 0.5.0 - Animation System

## 1. Design

All animation state is time-based. The source of time is `TIME_MS` (1 kHz
kernel uptime); no animation uses a frame counter or a fixed delay loop. A late
or skipped frame therefore advances the animation by the real elapsed time and
the animation still finishes at its designed moment.

The reusable primitives live in `shared/aster-motion` and are unit-tested on the
host:

| Primitive | Purpose |
|---|---|
| `Easing` | Linear, EaseIn, EaseOut, EaseInOut, CubicIn/Out/InOut, Smoothstep, Spring overshoot |
| `Tween` | Scalar from→to with start/duration/easing, progress, completion, retarget |
| `Spring` | Critically damped spring with stable sub-stepping |
| `MotionSystem` | Named owner/channel tracks, replacement, cancellation, collection |
| `Sequence` | Absolute-time multi-step sequence with `(index, progress)` |

`Tween` and `Spring` are deterministic pure functions of time. `MotionSystem`
allocates only when a track starts and removes tracks after completion.

## 2. Window transitions

`wm::Transition` stores:

```
from: Rect, to: Rect,
opacity_from, opacity_to,
start, duration, easing, kind
```

Kinds: Open, Close, Minimize, Restore, Maximize, Unmaximize, Snap.

- Open: 200 ms CubicOut, 96% rect → target, opacity 0 → 1.
- Close: 180 ms CubicIn, 100% → 96% around the centre, opacity 1 → 0.
- Minimize: 260 ms CubicInOut to the owning app's dock icon, 20×20 target,
  opacity 1 → 0.
- Restore: 260 ms CubicOut from the dock target to the saved normal rect,
  opacity 0 → 1.
- Maximize/unmaximize: 230 ms CubicInOut between the current and work-area rect.
- Snap: 210 ms CubicOut to the half-screen target.
- Focus: 170 ms CubicOut shadow strength interpolation.

Durations are multiplied by the persisted animation-speed factor and become
zero when Reduce Motion or the corresponding category switch is off. A zero
duration applies the final state immediately; no animation state is left behind.

## 3. Controls

- Start: create a `Transition`/`Tween`.
- Pause/resume/reverse: available through the primitive API; window transitions
  are intentionally short and currently do not expose pause/reverse in the UI.
- Cancel: replaced or cancelled when a new interaction starts (drag/resize), a
  window closes, or Reduce Motion is enabled.
- Completion: the window manager applies the final state in `tick`; the old area
  is marked dirty and closed windows are removed.
- Sequence/parallel: supported by `Sequence` and independent tracks; dock,
  workspace and UI tracks can run concurrently.

## 4. Rules enforced

- No animation blocks input: hit testing uses target rectangles; input is routed
  while transitions play.
- Window removal happens after the closing transition, and the removed area is
  repainted. Minimize completion forces a full cleanup repaint.
- Animations never own pointers to freed windows; state lives inside the window
  record and is dropped with it.
- Reduced Motion zeroes geometry/opacity durations and makes overview/switcher/
  menu/toast fades immediate.
- Animations are visually bounded: no infinite motion except the live Settings
  preview, which is explicitly a preview and stops when the page is not drawn.

## 5. What is not animated yet

- Buttons, checkboxes, toggles and slider thumbs use immediate state colors.
- Dialog/modal backdrops have no transition.
- Tab reordering is not implemented.
- Window group transitions do not exist.
