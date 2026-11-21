# Aster OS 0.5.0 - Dock 2.0

## 1. Position and geometry

The dock supports `Bottom`, `Left` and `Right`. Geometry is computed from real
icon/slot sizes:

- bottom: width = slots × 10 + 24, height = icon + 18;
- left/right: width = icon + 18, height = slots × 10 + 24;
- work area reserves the dock only when auto-hide is off.

When the position, size or icon setting changes, the window manager receives new
dock anchor points every frame. Minimize/restore animations therefore target the
current icon rather than a stale dock centre.

## 2. Magnification

Cursor proximity is measured along the dock axis. Icon scale is:

```
scale = 1 + magnification × proximity²
```

with `magnification` persisted from 0 to 1. Neighbouring icons react to the
cursor because proximity is calculated per slot. Scale is applied around the slot
centre, so the layout does not jump.

Reduced motion forces scale 1; the dock animations switch forces immediate
highlighting without magnification.

## 3. Auto-hide

When enabled, the dock slides off-screen after the pointer leaves the dock area
and edge zone. It slides back when the pointer reaches the screen edge or enters
the dock. The transition is time-based (`dt / 140 ms`) and is immediate under
Reduced Motion or with dock animations disabled. Hidden dock geometry is still
used for edge hit testing so the pointer can reveal it.

## 4. Running and minimized indicators

- A running dot is drawn for any app with a live window.
- A second small dot appears when that app has at least one minimized window.
- The indicator position follows bottom or side orientation.
- Indicators can be disabled in Graphics & Motion.

## 5. Settings

Persisted compactly as `dock=position,size,magnification,autohide,indicators`
inside the versioned settings file. The Settings page exposes bottom/left/right,
size, magnification, auto-hide and indicators; every control applies immediately
and is stored through the existing configd service with direct-I/O fallback.

## 6. Limits

- No drag-and-drop of icons or custom ordering.
- No separate dock for multiple displays.
- No bounce animation on launch; the window open transition is the feedback.
- No per-app badge counters.
