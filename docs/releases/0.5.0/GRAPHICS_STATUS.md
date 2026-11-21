# Aster OS 0.5.0 - Graphics Status

## 1. Implemented

- Software graphics engine modules and clear layer ownership.
- Dirty-region rendering with bounded merging and full-screen fallback.
- Partial framebuffer presentation.
- Real surface opacity for window fade.
- Time-based frame pacing, delta timing, FPS and frame statistics.
- Shared motion library with unit tests and cancellation/collection.
- Window state machine and open/close/minimize/restore/maximize/unmaximize/snap
  transitions; minimize targets the real dock icon.
- Focus shadow transition.
- Dock position, size, magnification, auto-hide and indicators.
- Workspace slide and indicators.
- Window overview with captured real previews and keyboard/mouse selection.
- Held-Alt quick switcher with release-to-switch.
- Text editor tabs with create/select/close and shortcuts.
- Menu open/close fade, toast slide/fade, reduced-motion handling.
- Settings > Graphics & Motion with live controls, persistence and a real preview.
- Monitor graphics telemetry: FPS, frame/draw/present, dirty pixels, redraws,
  animations and windows.
- Quality modes with real shadow and dirty-region effects.
- Persisted settings in the existing versioned service.

## 2. Partial

- Tabs: no drag reorder, tear-out or window groups.
- Smooth scrolling: Text Page Up/Down only; no wheel events in the PS/2 path.
- UI motion: menus/toasts/window/dock animate; buttons/toggles/sliders/dialog
  backdrops remain immediate.
- Effects: blur is precomputed wallpaper blur behind glass, not live backdrop blur.
- Resource management: ownership-based, no central cache/eviction/leak detector.
- High-DPI/multi-monitor: data structures exist; only one 1.0-scale display is real.
- Overview previews are captured when opening; they do not stream live video.

## 3. Not implemented

- GPU backend, shaders, hardware blit or composition.
- VSync/refresh-rate synchronization.
- Triple buffering.
- Rotation, multi-display window placement, runtime resolution changes.
- Window groups, tab tearing, drag-reordered tabs.
- High-contrast mode and screen-reader support.
- Live backdrop blur.
- Damage tracking inside app surfaces (dirty regions are compositor-level only).

## 4. Known issues

- Overview opening is full-screen and can cost tens of milliseconds under TCG.
- Large numbers of dirty rectangles are coalesced rather than tracked as an
  arbitrary region list; busy animations may fall back to a full redraw.
- Settings test expectations changed to the new compact graphics groups.
- QEMU screenshots are the visual regression source; there is no automated
  anti-flicker/video comparison.
