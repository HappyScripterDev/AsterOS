# Aster OS 0.5.0 - Accessibility

## 1. Motion settings

| Setting | Real effect |
|---|---|
| Reduce Motion | Zeroes window/dock/workspace/UI animation durations, makes overview/switcher/menu fades instant, disables toast slide, and snaps smooth scrolling |
| Animation speed | Scales all window, dock, workspace and UI durations (0.25×–2×) |
| Window animations | Enables/disables open, close, minimize, restore, maximize and snap transitions |
| Dock animations | Enables/disables magnification and auto-hide interpolation |
| Workspace animations | Enables/disables the desktop slide and indicator motion |
| UI animations | Enables/disables menu/toast/overview/switcher fades |
| Smooth scrolling | Enables time-based Text editor page scrolling |
| Focus indicators | Accent outline on selected overview/switcher items and window focus shadow |

Animations never disable functionality: with Reduce Motion, all window,
workspace, dock and overview actions still work and complete immediately.

## 2. Motion and readability

- No animation changes text content or makes text unreadable.
- Toast fades keep the content visible for most of the six-second lifetime.
- Overview dimming keeps cards bright and labels dark.
- Switcher labels use dark text on a light panel.

## 3. Effects accessibility

Transparency, blur and shadows can be disabled independently. The solid panel
fallbacks preserve contrast and borders. There is no High Contrast theme or
forced-colors mode yet.

## 4. Input and focus

- The cursor is drawn last and is not animated.
- Hit testing uses target window rectangles, so dragging and clicking do not wait
  for transitions.
- Keyboard: F3/Ctrl+Up overview, arrows select, Enter opens, Esc closes;
  Alt+Tab switcher; Ctrl+Left/Right desktops; Ctrl+Space launcher; settings
  up/down navigation.
- Focus is visible through the window shadow/title bar and overview outline.

## 5. Not implemented

- UI scale control at runtime (the setting exists for future backends; the UI
  still renders at 1.0).
- High-contrast mode.
- Screen-reader/accessibility tree.
- Keyboard focus ring on every button/toggle.
- Reduced-transparency system preference beyond the manual switch.
