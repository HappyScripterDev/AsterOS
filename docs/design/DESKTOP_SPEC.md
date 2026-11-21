# Aster OS Desktop Specification

Status: normative v1. Units are logical pixels at UI scale 1.0. Color, type, space,
radius, elevation and motion tokens come from DESIGN_SYSTEM.md.

Reference screen: 1280 x 800. The compositor is software rendered, 60 FPS cap, no
runtime blur except the pre-blurred wallpaper buffer.

---


> **Implementation geometry (shipped 2026-11-20).** At 1280x800: menu bar
> 26 px; dock 68 px tall with 62 px slots and 50 px icons (up to 1.45x
> magnification); Spotlight panel 660 px wide at y=96, dynamically sized to
> the result count; Control Centre 322x336 px at the top right; notifications
> panel 356 px wide and dynamically tall; window title bar 28 px, radius 12 px;
> traffic lights 12 px with 20 px spacing. The shell is light-only.


## 1. Reference geometry

```
x=0                                                          x=1280
+----------------------------------------------------------------+ y=0
| TOP BAR  28 px, glass                                          |
+----------------------------------------------------------------+ y=28
|                                                                |
|   WORK AREA  1280 x 692                                        |
|   windows live here, maximized to 0,28 1280x692                |
|                                                                |
|                                                                |
|                                                                |
|                                                                |
|                                                                |
+----------------------------------------------------------------+ y=720
|  8 px gap                                                      |
|  +----------------------------------------------+              | y=728
|  | DOCK  448 x 64, centered at x=416            |              |
|  +----------------------------------------------+              | y=792
|  8 px gap                                                      |
+----------------------------------------------------------------+ y=800
```

| Region | Rect | Notes |
|---|---|---|
| Top bar | 0, 0, 1280, 28 | Glass, always visible, above windows |
| Work area | 0, 28, 1280, 692 | Windows, snap, panels |
| Dock | 416, 728, 448, 64 | Centered, 8 px gap to work area and screen bottom |
| Dock hit region | 416, 718, 448, 74 | Includes the 2 px icon overflow into the work area |
| Bottom safe gap | 0, 720, 1280, 8 | No window content |

Rules:

- The top bar never overlaps a window; windows start at y=28. The dock background never
  overlaps a window; a magnified icon may reach y=718, which is 2 px into the work area
  at 1280 x 800.
- The 64 px dock is the background container. A magnified 40 px icon (max 62 px) may
  extend up to 10 px above the container, but never above y=718.
- Desktop has no icons in v1. Files is the only file browser entry point.
- Fullscreen exists as a distinct state: the window covers (0, 0, W, H), the top bar
  and dock are hidden while it is focused, and its app content receives the entire
  screen. Maximize still means the work area with chrome visible.

---

## 2. Coordinate and scale rules

- The compositor draws in physical pixels. All specs are logical; multiply by the UI
  scale S, round layout metrics to the nearest physical pixel.
- 1 px hairlines stay 1 physical pixel at every scale.
- Text bitmaps are scaled by S (DESIGN_SYSTEM.md 10.5).
- Minimum logical desktop: 960 x 600. Scaling is refused below that.
- At larger screens the layout constants do not grow: top bar 28, dock 64, launcher 720,
  control center 320, notification center 360. Only the work area grows.
- Window sizes grow by formula (section 6.2), then cap.

### 2.1 Layer order

| Z | Layer | Notes |
|---|---|---|
| 0 | Wallpaper | Full screen |
| 10-39 | Windows | Stacking order, focused on top |
| 40 | Dock | Above windows; window shadows may pass under it |
| 50 | Top bar | Above windows and dock |
| 60 | Launcher backdrop | Covers work area and dock, not top bar |
| 61 | Launcher panel | Glass |
| 62 | Control center / notification center | Opaque, mutually exclusive |
| 63 | Alt-Tab switcher | Opaque |
| 68 | Toasts | Above panels, below menus |
| 70 | Menus, context menus | Opaque |
| 71 | Tooltips | Topmost transient |
| 90 | App modal dialogs | Window-local composition, not a global layer |
| 100 | Lock screen | Not implemented; no UI |

Only one of launcher, control center, notification center, alt-tab switcher or a menu is
open at a time. Tooltips may appear above any of them. Clicking outside a transient
surface dismisses it.

---

## 3. Wallpaper

- Drawn once per resolution/theme change with the recipe in DESIGN_SYSTEM.md 6.4.
- Never parallax, never zoom, never animate.
- The pre-blurred copy is derived from it (section 6.3 of the design system).
- Theme switch crossfades the wallpaper over 320 ms; the pre-blur buffer is rebuilt
  before the crossfade starts.
- No wallpaper picker in v1 beyond the two theme gradients. Settings may show the
  current theme preview only.

---

## 4. Top bar

Height 28. Glass (DESIGN_SYSTEM.md 6). Bottom edge border 1 px
`rgba(255,255,255,0.06)` dark / `rgba(18,22,31,0.08)` light.

### 4.1 Layout at 1280

```
x=12  24       208                         1100   1142 1154 1174 1182 1202 1214 1268
|     |        |                             |      |    |    |    |    |    |    |
[star] Files   File  Edit  View  Window  Help  . . . .  [cc] [bell]      22:41
```

| Element | Rect | Details |
|---|---|---|
| Star mark | 12, 8, 12, 12 | 4-point mark, `color.text`, hit box 4,0,28,28 |
| App name | 32, 0, 160, 28 | 13/18 600, ellipsis, vertical center |
| Menu titles | start x=208, height 28 | 12/16 500, 8 px side padding, hover pill 24 high r=6 inset 2 |
| Pager | 1100, 12, 42, 4 | 4 marks; active pill 12x4 accent; inactive 4x4 secondary at 55% |
| Control center | 1154, 4, 20, 20 | Hit 28x28; sun/slider glyph |
| Bell | 1182, 4, 20, 20 | Hit 28x28; unread dot 4 px at 1196,6 |
| Clock | 1214, 0, 54, 28 | 12/16 500 tabular, right aligned, right edge 1268 |

Right inset is 12. Clock tooltip shows the full date at 12/16.

Desktop focused: app name "Aster", menu titles hidden. A window focused: app name is
the app's name and the app's menu titles appear.

### 4.2 Behavior

- Clicking the star mark opens the launcher (toggles if already open).
- Clicking the app menu opens the menu (section 12).
- Hovering a menu title while another menu is open switches menus after 120 ms.
- Clicking the bell toggles the notification center; clicking the control center icon
  toggles the control center. Opening one closes the other and the launcher.
- Clock is not interactive; hover tooltip is the only affordance.
- The pager is both an indicator and a control: each mark has a 16 x 16 hit target,
  clicks jump to that desktop, and the active mark grows to 12 x 4. Desktop switching
  also works with Ctrl+Left/Right.
- The top bar never auto-hides, but a fullscreen window covers it by design. Escape,
  Meta+F or F11 exits fullscreen.
- If a menu is open and the user clicks the desktop, the menu closes and the click is
  consumed.

### 4.3 Menu titles

Default set, left to right: File, Edit, View, Window, Help. Apps may omit a menu they do
not support; they may not add a menu that implies an unimplemented feature (Store,
Browser, Update, Installer).

Shared items:

- File: New Window (multi-instance apps only), Close Window (Ctrl+W), Quit App.
- Edit: Undo, Redo, Cut, Copy, Paste, Select All. Only present for apps with text.
- View: app-specific, no zoom controls in v1.
- Window: Minimize (Ctrl+M), Zoom, Enter/Exit Fullscreen (Meta+F or F11),
  Next Window (Ctrl+Tab), separator, Close (Ctrl+W).
- Help: About <App> (modal).

Disabled items use 38% alpha and do not open. Menu items never say "coming soon".

---

## 5. Dock

### 5.1 Geometry

Resting: 448 x 64 at (416, 728), radius 18, glass, `elev.3`.

Construction: side padding 12, nine 40 px icons, 8 px gaps.

The dock's hit region follows its animated width; the table above lists the resting
rect. The vertical hit region is 718..792 while any icon is magnified.

```
416                                                            864
+--------------------------------------------------------------+
|  [40]  [40]  [40]  [40]  [40]  [40]  [40]  [40]  [40]        | 64
|   .                                                          |
+--------------------------------------------------------------+
```

Icon centers at rest: x = 448 + 48*i, i = 0..8. Center y = 760.

Order is fixed in v1 (no reordering, no drag):

| i | App |
|---|---|
| 0 | Files |
| 1 | Terminal |
| 2 | Text |
| 3 | Notes |
| 4 | Calendar |
| 5 | Photos |
| 6 | Calculator |
| 7 | Monitor |
| 8 | Settings |

### 5.2 Magnification

For each item i, with pointer x `p` and resting center `c_i`:

```
d = abs(p - c_i)
f = d < 96 ? 0.5 * (1 + cos(pi * d / 96)) : 0
target_scale = 1 + 0.55 * f
```

Distance is measured from the resting center, never the animated center, so the
magnification cannot chase the pointer.

Smoothing per frame:

```
s += (target_scale - s) * (1 - exp(-dt / tau))
tau = 0.06 / animation_speed     when animation_speed > 0
tau = 0                          when animation_speed = 0 (instant)
```

`dt` is seconds, clamped to 0.032. The result is clamped to 1.0..1.55. With reduce
motion on, `s = 1.0` immediately and the hover lift is a fixed 2 px.

Icon sizing and layout:

- Icon rect: `40 * s`, bottom edge fixed at y=780, horizontally centered in its slot.
- At max scale the icon is 62 x 62, top edge y=718. It may extend 10 px above the dock.
- Slot width is `40 * s`; gap stays 8; side padding stays 12.
- Dock width = `24 + sum(40 * s_i) + 8 * 8`, centered on the screen.
- Dock background remains 64 px tall. Icons are not clipped by it.
- Hit testing uses the visible icon rect. The dock's input region extends to y=718 only
  while an icon is magnified, so the overflowing top of an icon remains clickable.

The dock width grows by at most about 70 px with adjacent icons magnified. It never
exceeds 560 px in v1.

With reduce motion on, magnified icons stay at 1.0 scale and lift 2 px; the dock width
does not change.

### 5.3 Indicators

- Running, unfocused: 4 x 3 dot, radius pill, `color.text-secondary`, centered at
  y=783, 1 px below the icon bottom.
- Focused: 12 x 3 pill, radius pill, `color.accent`.
- Not running: no indicator.
- The indicator does not scale with magnification; it stays 3 px tall and is drawn
  under the enlarged icon.

### 5.4 Interaction

- Hover: magnify, show a tooltip after 500 ms above the icon (6 px gap).
- Click a running app: raise and focus its most recent window; if it is already the
  focused window, minimize it. If the app has windows on another desktop, switch to
  that desktop (320 ms) and focus the window.
- Click a non-running app: launch it, using the app's single-instance rule.
- Launch opens the window at its default size on the current desktop.
- Right-click: no menu in v1. Do not show a disabled menu.
- The dock is not in the keyboard tab order. Launcher search and shortcuts are the
  keyboard equivalents.
- Icon press: scale 0.96 for 120 ms in addition to magnification, then release.
- Minimized windows animate to their app icon's current rect, not to a fixed point.

### 5.5 Dock overflow

Fixed nine apps, no overflow in v1. If the app set grows, the dock must not shrink icons
below 40 px; use a second row or a scroll affordance, specified before implementation.

---

## 6. Windows

### 6.1 Frame

```
+--------------------------------------------------------------+  ^
| 28 px titlebar: [o][o][o]              title          [o][o][o]|  | 28
+--------------------------------------------------------------+  v
| window body: color.surface                                    |
|                                                               |
|                                                               |
+--------------------------------------------------------------+
```

- Radius 14, border 1 px `color.window-border` focused /
  `color.window-border-inactive`, shadow `elev.4` / `elev.4-inactive`.
- Content starts 28 px below the frame top. No additional toolbar padding is added by
  the frame; apps own their 44 px toolbar.
- Window body is opaque `color.surface`. Windows are never glass.
- Window shadow is not part of the frame rect and is not hit-testable.

### 6.2 Size and placement

```
base_w = 880 + (workW - 1280) * 0.25
base_h = 560 + (workH - 692) * 0.25
w = clamp(base_w, 720, 1120)
h = clamp(base_h, 480, 720)
w = min(w, workW - 8)
h = min(h, workH - 8)
```

At 1280 x 800 this yields 880 x 560 at (200, 94). Center it in the work area, then add
24 px to x and y for each successive new window; wrap to the centered base after six
windows. Never place a window so its titlebar is outside the work area.

Minimum window: 360 x 240. Maximum: the work area (manual resize may fill it; the
default size keeps an 8 px inset). Apps may request a smaller default (Calculator
360 x 480) as long as it is >= the minimum.

### 6.3 Titlebar

- Height 28. Drag region is the full titlebar except the control group.
- Title: 13/18 600, centered on the window's horizontal center, max width
  `window.width - 200`, ellipsis, `color.text` focused / `color.text-secondary`
  unfocused.
- Window controls, right aligned, padding 12:
  - three 12 px circles, 8 px gap, centers at W-58, W-38, W-18, center y 14.
  - order left to right: minimize, maximize/restore, close.
  - rest fill: `#2A3140` dark / `#D8DEE8` light, with a 1 px inset
    `color.border-control` edge (3:1 boundary), no glyph.
  - hover fill: minimize `#FBBF24`, maximize `#34D399`, close `#F87171`, no outline;
    glyph `#0B0D12` at 1.5 px stroke, drawn only while the control group is hovered.
  - hit box per control: 20 x 28, full titlebar height. Group spans W-68 to W-8.
  - tooltips: "Minimize", "Maximize" / "Restore", "Close" after 500 ms.
- Double-click on the drag region toggles maximize. Double-click does not apply to the
  control group.
- Right-click on the titlebar: no menu in v1.

The 20 x 28 hit boxes are below the 28 x 28 system target. This is a documented
exception covered by WCAG 2.2 SC 2.5.8's equivalent-control exception: every control
has a >= 28 px menu item and a keyboard shortcut (Super+M, Super+Up, Super+Q).

### 6.4 Resize

- Invisible 6 px hit inset on each edge, 12 x 12 corners. Corners win over edges.
- Cursor changes when the pointer is inside the hit zone: `ns-resize`, `ew-resize`,
  `nwse-resize`, `nesw-resize`.
- Resize is live. Clamp to 360 x 240 minimum and the work area maximum.
- Resizing a snapped or maximized window is not possible until it is restored.
- Resize edges do not show a visual handle on hover. The cursor is the only cue.
- While resizing, the window keeps `elev.4`; no scale animation.

### 6.5 Focus, raise, close

- Click anywhere in a window: focus it, raise to the top of the window stack, and
  deliver the click to the target.
- Hover does not focus.
- Focus moves to the next window in MRU order on minimize or close. If none, focus
  returns to the desktop.
- Close is immediate. Apps with unsaved changes must present an in-window modal
  (section 13.4) before closing.
- Single-instance apps: Settings, Calculator, Monitor, Calendar, Photos. Multi-instance:
  Files, Terminal, Text, Notes.
- A new window opens on the current desktop.

### 6.6 Window open, close, minimize

- Open: scale 96% -> 100% (0.96 -> 1.00) and opacity 0 -> 1, 200 ms enter, origin
  window center.
- Close: scale 100% -> 96% and opacity 1 -> 0, 120 ms exit.
- Minimize: interpolate the full window rect to the dock icon rect, radius 14 -> 10,
  fade to 0, 200 ms standard. The window is removed from input immediately at t=0.
- Restore: reverse, 200 ms enter, focus on completion.
- Maximize / snap: rect interpolation, 200 ms standard. Radius 14 -> 0 only for
  maximize; snapped windows keep 14.
- During open/close/minimize animations the window is not clickable except that
  minimize does not block clicking its dock icon again.

---

## 7. Snap zones

Only three targets in v1: left half, right half, maximize. Quarter snap is not
implemented and must not be advertised.

```
+---------------------------+  8  +---------------------------+
| LEFT                      |     | RIGHT                     |
| 636 x 692                 |     | 636 x 692                 |
|                           |     |                           |
+---------------------------+     +---------------------------+
        drag to x <= 8                 drag to x >= 1272
        release -> snap               release -> snap

top: drag to y <= 36 (top bar + 8) -> maximize preview -> release -> maximize
```

Targets at 1280:

| Target | Rect |
|---|---|
| Left half | 0, 28, 636, 692 |
| Right half | 644, 28, 636, 692 |
| Maximize | 0, 28, 1280, 692 |

General formula: `half = (workW - 8) / 2`; left is `(0, 28, half, workH)`, right is
`(half + 8, 28, half, workH)`.

Behavior:

- While dragging a window, if the pointer is within 8 px of the left or right work-area
  edge, start a 120 ms dwell. If it stays, show the preview and snap on release.
- Top target: pointer y <= 36 while dragging. Same 120 ms dwell.
- Leaving the zone by more than 16 px cancels the preview (hysteresis).
- Preview: target rect filled with accent at 14% alpha, 2 px accent stroke inset 1,
  radius 14 (0 for maximize). No animation beyond a 120 ms fade.
- Only one preview at a time. Preview is not interactive.
- Dragging a snapped or maximized window by the titlebar more than 8 px restores it to
  its pre-snap rect, preserving the pointer's relative x position on the titlebar, then
  continues the drag.
- Keyboard: Super+Left/Right snaps, Super+Up maximizes/restores, Super+Down restores.
- Snapped windows fill the work area edge; the 8 px gutter shows wallpaper.
- There is no bottom-half snap in v1.

---

## 8. Virtual desktops

- Four desktops, fixed, no dynamic add/remove.
- Each desktop owns a window list and a z-order. Wallpaper and chrome are shared.
- Ctrl+Left / Ctrl+Right switch. Clicking a top-bar pager mark jumps directly. Clamp at
  desktop 1 and 4: no wrap, no bounce. At an end, the pager pulses once (120 ms) with no
  other effect.
- Transition: outgoing windows translate -48 px and fade to 0.25; incoming windows
  translate from +48 px to 0 and fade to 1. 320 ms standard. Top bar and dock do not
  move. Direction reverses for Right.
- Reduce motion: instant cut.
- New windows open on the current desktop.
- Dock click finds the app's most recent window on any desktop and switches to that
  desktop before focusing.
- Window Overview (F3 or Ctrl+Up) shows every window on the current desktop as a live
  card; arrows select, Enter focuses, Escape closes, clicking a card focuses it.
- The pager is centred on the top bar: marks are 14 px apart, active 12 x 4 accent pill,
  inactive 7 x 4 at 60/255 black. Its hit targets are 16 x 16.
- Drag-window-to-desktop and desktop reordering are not implemented in v0.5.

---

## 9. Launcher

### 9.1 Open and backdrop

Triggers: tap Super; Space when no window has focus and no text field is focused;
click the star mark.

- The launcher is modal. It covers the work area and the dock.
- Backdrop: draw the pre-blurred wallpaper at 100% over 0,28,1280,772, then the
  launcher scrim. Windows are not visible while the launcher is open; this is a
  deliberate consequence of the pre-blurred wallpaper pipeline (no runtime blur of
  window content).
- The dock is covered by the backdrop as the scrim fades in. The top bar remains visible;
  the backdrop begins at y=28.
- Clicking the backdrop closes the launcher. Clicking the top bar closes the launcher
  and performs the top-bar action.

### 9.2 Panel

```
                        280                        1000
                        +---------------------------+
                        | [search 680 x 44]         |  y=96
                        |                           |
                        | [120x104] [120x104] ...   | 5 columns
                        | [120x104] [120x104] ...   | 2 rows
                        +---------------------------+
                        720 x 324, radius 24, glass
```

- Panel: x = (W - 720) / 2, y = 96, width 720, height 324. Max height is
  `min(520, workH - 76)`. At 960 x 600 (150% on 1440 x 900) the max is 416, so results
  scroll rather than overflow. Radius 24. Glass. `elev.3`.
- Search: x=20, y=20, 680 x 44, radius 10, fill `rgba(255,255,255,0.06)` dark /
  `rgba(0,0,0,0.04)` light, 1 px `border-control`, 1 px inner highlight. Search icon 16
  at x=34, centered. Placeholder "Search apps" at x=58, 13/18, `text-secondary`. Text
  starts at x=58. Clear button 20 x 20 at x=668 when the query is non-empty, hit 28.
- Grid: 5 columns, tile 120 x 104, gap 16, block 664 x 224, at x=28, y=80.
- Tile: icon 32 at top 16, label at y=64, 12/16 500, centered, max width 104, ellipsis.
  Hover fill `state.hover-overlay`, radius 14. Focus ring 2 px inset 2.
  Press scale 0.97 for 120 ms.
- Panel animation: scale 0.98 -> 1, opacity 0 -> 1, 120 ms enter. Scrim 200 ms.

### 9.3 Keyboard

- On open, focus the search field and show a caret. The ring is not drawn for the field
  until keyboard input, per focus policy.
- Typing filters immediately. Backspace edits. Esc closes the launcher and returns
  focus to the previous window (if any).
- Up/Down move between grid rows; Left/Right within a row; wrap at edges.
- Enter launches the focused tile. If the search field is focused and has text, Enter
  launches the first result.
- Tab moves focus from the search field to the first tile and cycles; Shift+Tab reverses.
- Focus is trapped inside the launcher.

### 9.4 Search and results

- Match: case-insensitive. Prefix matches rank above substring matches. Ties keep dock
  order. No fuzzy matching, no transliteration.
- Matches: app display name and a small keyword list (for example "settings",
  "preferences"; "files", "finder" is not used).
- Empty query: 5 x 2 tile grid, all nine apps.
- Non-empty query: replace the grid with a result list, x=20, width 680, rows 36,
  radius 6, max 5 visible. Panel height = 20 + 44 + 12 + 24 + (rows * 36) + 20,
  clamped to 260..`min(520, workH - 76)`. Group label "Applications" 11/14 500 at y=76.
- No results: search icon 32, "No apps found" 15/20 600, `"<query>"` 13/18 secondary,
  centered in a 180 px tall body. No action button.
- Query resets when the launcher closes.
- Launching: close the launcher (120 ms), then open or focus the app. Single-instance
  apps focus their existing window, switching desktops if needed.

---

## 10. Control center

Opaque panel. 320 x 312 at (W - 328, 36) = (952, 36) at 1280. Radius 18,
`surface-elevated`, 1 px `color.panel-border`, `elev.3`.

```
+--------------------------------+
| Control Center            [x]  | 40   y=16
| [ Theme: Dark ] [Reduce motion]| 72   y=64
| [ DND         ] [ Night light ]| 72   y=144
| Animation speed      [--o--] 1x| 28   y=232
| Night light warmth   [--o--] 40%| 28  y=272
+--------------------------------+
```

- Header: 40 px, title 13/18 600 at x=16; close button 20 at (284, 26), hit 28 x 28 at
  (280, 22).
- Toggle tiles: 140 x 72, radius 14, gap 8, x=16 and 164, y=64 and 144.
  - Off: fill `rgba(255,255,255,0.04)` dark / `rgba(0,0,0,0.03)` light, 1 px hairline.
  - On: fill accent at 18%, 1 px accent at 40%, icon accent, label `color.text`.
  - Icon 20 at top 12, centered. Label 11/14 500 centered at y=46, max width 124.
  - Contents: Theme (label "Theme: Dark" or "Theme: Light", sun/moon icon, click
    toggles), Reduce motion, Do not disturb, Night light.
  - Night light is a warm color transform on the final framebuffer, not an alpha
    overlay. It must not push text below its AA contrast ratio.
  - Press scale 0.98. Focus ring inset 2.
- Sliders: rows 28, y=232 and 272, label 72 (12/16 500), track from x=96 to 256,
  value 40 right aligned at x=264, 11/14 secondary tabular.
  - Animation speed: 5 stops Off, 0.5x, 1x, 1.5x, 2x; default 1x.
  - Night light warmth: 0..100 in steps of 5; enabled only when Night light is on,
    otherwise 38% alpha and not focusable.
  - Track 4 px `hairline`; filled portion accent; knob 14 px circle `color.text` with
    1 px `rgba(0,0,0,0.20)` ring. Hit height 28.
- No volume or brightness controls: v1 has no audio stack and no panel backlight
  control. Do not add dummy sliders, disabled or otherwise.
- Open: y -4 -> 0, fade, 120 ms enter. Close: 120 ms exit. Anchored at top right.
- Click outside or Esc closes. Opening the notification center or launcher closes it.
- Toggling a setting applies immediately and persists. No "Apply" button.

---

## 11. Notification center and toasts

### 11.1 Notification center

360 wide at (W - 368, 36) = (912, 36). Radius 18, `surface-elevated`, panel border,
`elev.3`. Height dynamic, max `min(640, workH - 16)` = 640 at 1280, so the bottom stays
inside the work area.

```
+------------------------------------+
| Notifications            Clear all | 44
| +--------------------------------+ |
| | (i) Title              2m   [x] | | 64+
| |     Body, max 2 lines          | |
| +--------------------------------+ |
| +--------------------------------+ |
| | (i) Title              5m      | |
| +--------------------------------+ |
+------------------------------------+
```

- Header 44: title 15/20 600 at x=16; "Clear all" 12/16 500 accent text at right x=16,
  hit 28. Clears all immediately; no undo in v1.
- List padding 16, cards 328 wide, gap 8. Newest at top.
- Card: min height 64, radius 14, `color.surface` (dark) / `#FFFFFF` (light), 1 px
  hairline. Padding 12. Icon 20 at 12,12. Title 13/18 600 at x=44, max width 240.
  Body 12/16 `text-secondary`, max 2 lines. Timestamp 11/14 secondary at top right,
  reserve 40 px. Unread dot 4 px accent to the left of the title at x=40.
  Dismiss button 20 at top right, visible on card hover, hit 28.
- Max 6 cards visible, then vertical scroll; scrollbar 8 px.
- Card click opens the source app, focuses the relevant window, and closes the center.
- Empty: bell icon 32 at top, "No notifications" 15/20 600, "System and app alerts
  appear here." 13/18 secondary, max width 280, centered, 200 px body.
- Opening the center marks all notifications read and clears the top-bar unread dot.
- Esc, outside click, or the bell closes it. Opening control center or launcher closes
  it.
- No notification actions (reply, archive) in v1.

### 11.2 Toasts

360 wide at (912, 36), min height 72, max 96, radius 14, `surface-elevated`, panel
border, `elev.2`, z=68. Same inner layout as a card minus the unread dot.

Stacking rules:

- Newest on top, 8 px gap, maximum 4 visible. A fifth notification queues and appears
  when a slot frees. No overlap, no shingling.
- Toast top does not move when another toast is dismissed; entries below move up 200 ms.
- If the notification center is open, new notifications do not toast.
- Do not disturb: no toasts; everything is recorded in the center.
- Clicking a toast opens the source app and dismisses the toast.
- Auto-dismiss after 5000 ms. Hovering anywhere in the stack pauses all timers; keyboard
  focus inside a toast pauses that toast. Resume with the remaining time, not a reset.
- Dismiss button 20 visual / 28 hit at the top right, appears on toast hover.
- Enter: y -8 -> 0 + fade, 200 ms. Exit: reverse, 120 ms.
- No critical/sticky toast in v1; important errors also persist in the center.

---

## 12. Menus, context menus, tooltips

### 12.1 Menus

Menu bar dropdown:

- Anchor: x = menu title left, y = 32 (4 px below the bar).
- Width min 180, max 320, padding 4, radius 10, `color.surface`, 1 px panel border,
  `elev.2`.
- Item: 28 high, radius 6, padding 8. Icon 14 at x=8; label at x=28, 13/18 400.
  Shortcut right aligned at x=width-8, 11/14 secondary. Hover fill
  `state.hover-overlay`.
- Check items: 12 px check at x=8 instead of an icon. Radio items: 8 px dot.
- Separator: 1 px hairline, 4 px vertical margin, full inner width.
- Disabled: 38% alpha, no hover, not focusable.
- Submenu: opens after 120 ms hover, positioned x = parent right - 4, y = item top - 4.
  Max depth 2.
- Height max 320, then scroll. No menu item wraps; use ellipsis.
- Keyboard: Up/Down, Home/End, Enter/Space, Right opens submenu, Left closes, Esc
  closes all, type-ahead by first letter.
- Clicking outside closes. Opening a menu closes any panel or the launcher.
- Menu switching while open is hover-driven after 120 ms.

### 12.2 Context menus

- Same surface and metrics as menus.
- Position: 2 px right/below the pointer. Flip left if x + width > workW - 4; flip up
  if y + height > workH - 4. Clamp to the work area with a 4 px margin.
- Trigger: right-click. No long-press (no touch).
- Desktop menu: Open Launcher, New Terminal Window, Settings, separator, Next Desktop.
  All items must work; do not add disabled placeholders.
- Dock and titlebar have no context menu in v1.
- App content menus are app-defined; they must not contain unimplemented actions.
- Dismiss on outside click, Esc, scroll, or window focus change.

### 12.3 Tooltips

- Delay 500 ms on hover; 0 ms on keyboard focus of an icon-only control. Hide
  immediately on leave, press, scroll or window move.
- Max width 240, padding 6 x 8, radius 6, 12/16 500.
- Colors: `#1B202A` background, `#F2F4F8` text, 1 px `rgba(255,255,255,0.10)`, `elev.1`.
  Use the same dark tooltip in both themes (14.8:1).
- Placement: 6 px above the anchor, centered; flip below if it would cross y=32; clamp
  8 px from work-area edges.
- Max 3 lines, no rich content, no buttons, no links.
- Tooltips never contain the only copy of essential information.
- Do not show tooltips for disabled controls; explain the requirement with inline
  helper text instead.

---

## 13. Alt-Tab switcher

Opaque, 560 x 160, centered in the work area: (360, 294) at 1280. Radius 18,
`surface-elevated`, panel border, `elev.3`.

```
+--------------------------------------------------------------+
|  [72]   [72]   [72]   [72]   [72]                            |
|  Files  Text   Term   Notes  Calc                             |
+--------------------------------------------------------------+
```

- One row of up to 6 window tiles, 72 x 72, gap 12, centered. More than 6: horizontal
  scroll by arrows, no second row.
- Tile: icon 32 at top 12, label 12/16 at y=48, max width 64, ellipsis.
- Selected: fill accent 18%, 2 px accent ring radius 10, label `color.text`.
- MRU order, most recent first, for windows on the current desktop only. Focus does not
  change until commit.
- Hold Alt; Tab advances, Shift+Tab reverses, arrows move, release Alt activates,
  Esc cancels, click activates. Releasing Alt with no change activates the selected
  window (which may be the current one).
- Show even with one window; do nothing with zero windows.
- Opening/closing is instant (no scale) with a 120 ms fade.

---

## 14. Larger screens and scaling

At any size >= 1280 x 800:

| Constant | Value |
|---|---|
| Top bar | 28 |
| Dock | 448 x 64, centered, bottom 8 |
| Work area | W x (H - 108), origin (0, 28) |
| Launcher | 720 wide, centered, y=96 |
| Control center | 320 wide, right 8, y=36 |
| Notification center | 360 wide, right 8, y=36 |
| Toast | 360 wide, right 8, y=36 |
| Window default | formula in 6.2, cap 1120 x 720 |
| Sidebar | 200, fixed |
| Content padding | 20, fixed |
| List row | 36, fixed |

- Windows center in the work area, not the screen.
- The top bar left cluster stays at x=12; the right cluster anchors to W-12.
- The launcher grid does not add columns; it stays 5 columns and 720 wide.
- The dock never grows beyond nine 40 px icons.
- At W >= 1920 the work area is wider; apps stretch content, sidebars stay 200. Text
  measure in editors stays capped at 680.
- Scaling 125/150/200 follows DESIGN_SYSTEM.md 10.5. Minimum logical size 960 x 600.
  Below that, the scale option is disabled with a plain explanation; do not clip.

---

## 15. Transient surfaces and dismissal

Priority for Esc, highest first:

1. Menu / context menu
2. Tooltip (dismiss only; no focus change)
3. Control center / notification center
4. Launcher
5. Alt-Tab switcher (cancel)

Rules:

- Esc never closes a window.
- Clicking outside dismisses the top transient and consumes the click, except for the
  launcher, where clicking the top bar both dismisses and performs the bar action.
- Opening a new transient closes the previous one.
- Toasts are independent and do not take focus. Clicking one opens its source.
- Tooltips do not take focus and do not block hit testing.
- Menus are above panels; a menu opened from a panel closes the panel.

---

## 16. Pixel budget at 1280 x 800

| Region | Pixels |
|---|---|
| Top bar | 28 |
| Work area height | 692 |
| Dock height | 64 |
| Gaps around dock | 16 |
| Window height (default) | 560 |
| Window content height | 532 |
| App body after 44 px toolbar | 488 |
| Window width (default) | 880 |
| App content width after 200 px sidebar | 680 |
| App content width after 20 px padding each side | 640 |
| Launcher | 720 x 324 |
| Control center | 320 x 312 |
| Notification center | 360 x up to 640 |
| Toast | 360 x 72-96 |

---

## 17. Explicit non-implementations

Do not draw, list, disable, label "coming soon", or otherwise imply any of these in v1:

- Lock screen, lock button, Super+L UI (shortcut is reserved, see INTERACTION.md).
- Store, Installer, Update, Browser, app marketplace, download center.
- Network, Wi-Fi, Bluetooth, battery, volume, brightness controls.
- Multi-monitor, display arrangement, touch, gestures, pen.
- Desktop icons, folders on the desktop, drag-and-drop between apps.
- Screenshot, screen recording, window tabs outside Terminal, picture-in-picture.
- Notification actions, reply from toast, notification history search.
- RTL layout, screen reader bridge, voice control.
- Quarter snap, bottom-half snap, desktop overview, window tiling layouts beyond left,
  right and maximize.
