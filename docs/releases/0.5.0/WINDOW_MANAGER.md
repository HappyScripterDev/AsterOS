# Aster OS 0.5.0 - Window Manager

## 1. State machine

`WindowState`:

| State | Meaning |
|---|---|
| Opening | Open transition running |
| Normal | Fully visible, no geometry transition |
| Minimizing | Moving/fading to its dock icon |
| Minimized | Hidden, owned by the dock entry |
| Restoring | Moving/fading from the dock icon |
| Closing | Close transition running |
| Maximized | Normal geometry is the work area inset by 8 px |
| Snapped | Geometry is a half-screen snap target |

Invalid transitions are rejected: close is idempotent, minimize ignores an
already minimizing/minimized window, restore ignores a visible window, and a
new drag cancels an opening transition. Closing windows are excluded from hit
testing, focus selection and the overview.

Each window stores its target `rect`, saved `normal` rect, desktop, z-order,
snap code, minimized/maximized flags, current transition, focus timestamp,
title and cached shadow.

## 2. Operations

- **Open:** reuse the existing window of the same app and focus it; otherwise
  create a new window with a staggered position and run the open transition.
- **Close:** start the close transition, move focus to the highest remaining
  window, and remove the window after completion.
- **Minimize:** animate to the current dock icon centre for that app (position,
  size and magnification aware), then mark it minimized.
- **Restore:** animate from the dock icon to the saved normal rectangle and focus.
- **Maximize/unmaximize:** save the normal rect, animate to/from the work area.
- **Snap:** drag to the left/right edge or top edge, show a live preview, and on
  release animate to the half-screen/maximized target.
- **Move/resize:** direct interaction, cancelling any geometry transition; input
  is not delayed by animation.
- **Focus:** raises z-order, updates the focus timestamp and animates the shadow.

## 3. Workspaces

Three desktops. Switching changes the active desktop, updates focus, starts a
280 ms horizontal slide, and draws indicators. Windows keep their desktop
assignment and are skipped on other desktops. The overview shows the workspace
label and can switch to it when selecting a window.

## 4. Overview and switcher integration

- `overview_items()` returns live records: id, title, app kind, desktop,
  minimized flag and normal rect.
- Focus/restore uses `focus_window(id)`, switching desktops and restoring
  minimized windows as needed.
- The quick switcher builds from the same records, so it cannot show a stale
  hardcoded list.

## 5. Limits

- One window per app kind; there is no multi-window app model yet.
- No window groups or tab tearing.
- No alt-tab thumbnail animation beyond the switcher fade.
- Window position/size is not persisted across reboots.
- The virtual-desktop count is fixed at three.
