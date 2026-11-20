# Aster OS Interaction Specification

Status: normative v1. This file defines input behavior, component states, empty and
error presentation, and accessibility behavior. Geometry comes from DESKTOP_SPEC.md;
visual tokens come from DESIGN_SYSTEM.md.

Input devices in v1: mouse or trackpad with two buttons and a wheel, and a keyboard.
There is no touch, pen, gesture, or multi-touch input.

---

## 1. Keyboard shortcuts

"Super" is the Windows/Command/Logo key. Shortcuts are matched on keydown unless noted.
The compositor consumes Super, Ctrl+Alt, Alt+Tab, F10, Shift+F10 and Esc when a
transient is open. Apps receive all other keys.

### 1.1 Global shortcuts

| Shortcut | Action | Status |
|---|---|---|
| Super (press and release, no chord) | Toggle the launcher | v1 |
| Space | Open the launcher when the desktop has focus and no text field is focused | v1 |
| Super+Q | Close the focused window | v1 |
| Super+M | Minimize the focused window | v1 |
| Super+Up | Maximize / restore the focused window | v1 |
| Super+Down | Restore a snapped or maximized window; no-op when floating | v1 |
| Super+Left | Snap the focused window to the left half | v1 |
| Super+Right | Snap the focused window to the right half | v1 |
| Ctrl+Alt+Left | Previous virtual desktop; clamps at desktop 1 | v1 |
| Ctrl+Alt+Right | Next virtual desktop; clamps at desktop 4 | v1 |
| Alt+Tab | Open the switcher, advance selection | v1 |
| Alt+Shift+Tab | Open the switcher, move selection backward | v1 |
| Alt (hold) + arrows | Move selection in the switcher | v1 |
| Esc | Dismiss the top transient; cancel a window drag or resize | v1 |
| Tab / Shift+Tab | Move focus forward / backward | v1 |
| Enter / Space | Activate the focused control | v1 |
| Up / Down / Left / Right | Move within the focused composite widget | v1 |
| Home / End | First / last item in a list or menu | v1 |
| PageUp / PageDown | Scroll one page in a scrollable region | v1 |
| F10 | Focus the menu bar (first menu title) | v1 |
| Shift+F10 | Open the context menu for the focused element | v1 |
| Super+L | Lock screen | NOT IMPLEMENTED. Reserved. No lock UI, no menu item, no toast. |
| Super+D | Show desktop | NOT IMPLEMENTED |
| Super+Tab | Window switcher | NOT IMPLEMENTED; use Alt+Tab |
| Alt+F4 | Close window | NOT IMPLEMENTED; use Super+Q |
| PrintScreen | Screenshot | NOT IMPLEMENTED |
| Ctrl+Alt+Shift+Left / Right | Move window to another desktop | NOT IMPLEMENTED |
| Super+1..9 | Launch dock app by position | NOT IMPLEMENTED |

### 1.2 App shortcuts (pass-through)

The compositor does not consume Ctrl+letter combinations. Apps implement these
consistently when the action applies:

| Shortcut | Action |
|---|---|
| Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Ctrl+X / Ctrl+C / Ctrl+V | Cut / copy / paste |
| Ctrl+A | Select all |
| Ctrl+F | Find in the current document or folder |
| Ctrl+S | Save |
| Ctrl+O | Open |
| Ctrl+N | New document or window |
| Ctrl+W | Close the current tab or document, not the window |
| Ctrl+Home / Ctrl+End | Start / end of document |
| Shift+arrows | Extend selection |

Apps must not bind Super, Ctrl+Alt, Alt+Tab, F10, Shift+F10, or Esc at the window level.
Esc inside a text field first cancels an IME composition or closes an app popup, then
propagates to the compositor.

### 1.3 Repeat and timing

- Key repeat starts after 500 ms, then repeats every 33 ms for arrows, Home/End,
  PageUp/PageDown, Backspace and Delete. These are the defaults; Settings offers
  250/500/750 ms delay and 50/33/20 ms repeat rates.
- Shortcut combinations never repeat.
- Double-click: two presses within 500 ms and 3 px of each other.
- Drag threshold: 3 px.
- Wheel: 48 px per notch, vertical; Shift+wheel 48 px horizontal. No Ctrl+wheel zoom in
  v1.
- A held Alt for the switcher has no repeat; Tab repeats at the normal key repeat rate.

### 1.4 Shortcut discovery

- Shortcuts appear right aligned in menus.
- Tooltips name the gesture when a control has a shortcut (for example "Minimize
  (Super+M)").
- No shortcut cheatsheet overlay in v1. Settings > Keyboard > Shortcut reference opens
  the read-only list from 1.1.

---

## 2. Window behavior

### 2.1 Focus and raise

- Focus follows click, never hover.
- A click anywhere in an inactive window focuses and raises it, and is delivered to the
  control under the pointer in the same event. There is no "first click activates only"
  mode.
- The focused window is the top of the normal window stack. Menus, panels and the dock
  are not part of that stack.
- Focus is per window, not per app. Each window remembers its focused control.
- Clicking the desktop clears window focus.
- Minimizing or closing the focused window moves focus to the next window in MRU order.
  If none exists, focus returns to the desktop.

### 2.2 Move

- Press on the titlebar drag region (anywhere except the 60 px control group).
- Movement starts after 3 px of pointer travel. Below 3 px it is a click.
- The window follows the pointer 1:1 with the grab offset preserved. No inertia.
- The window frame is clamped to the work area: the pointer may move past an edge while
  dragging, but the frame stops at the boundary. This keeps the titlebar and resize edges
  reachable.
- Dragging a maximized or snapped window: after 8 px of movement, restore it to its
  pre-snap rect, keep the pointer at the same relative x position on the titlebar, and
  continue the drag.
- Esc during a drag restores the original rect and ends the drag.
- Release ends the drag. If the pointer is in a snap zone and the dwell completed, snap.

### 2.3 Resize

- 6 px invisible inset on each edge; 12 x 12 corners. The corner zone wins.
- The cursor changes on entering the zone (section 3).
- Resizing is live; no preview rectangle.
- Minimum 360 x 240. Maximum is the work area.
- Esc restores the original rect.
- A maximized window cannot be resized. A snapped window must be restored first.
- Resizing does not change the titlebar height or the content layout rules.

### 2.4 Maximize, snap, restore

- Double-click the titlebar drag region toggles maximize. On a snapped window, double
  click maximizes; double click again restores the pre-snap rect.
- Super+Up toggles. Super+Left/Right snap. Super+Down restores.
- The restore rect is the last floating rect, including position and size.
- Maximized: work-area rect, radius 0, no shadow, titlebar still 28.
- Snapped: half-work-area rect, radius 14, shadow `elev.4`.
- Dragging a snapped window restores it after 8 px (section 2.2).
- Window open and restore use the animations in DESKTOP_SPEC.md 6.6.

### 2.5 Close and minimize

- Super+Q and the close control close immediately. Apps with unsaved data must intercept
  the close request and show an in-window modal.
- Super+M and the minimize control animate to the dock icon, then remove the window from
  input and the focus order.
- Closing does not quit the app unless it is the app's last window and the app is
  single-window (Calculator, Settings, Monitor, Calendar, Photos).
- There is no "quit app" gesture in the UI beyond closing all of its windows. The File
  menu may provide Quit App; it closes every window of that app after the same unsaved
  checks.

### 2.6 Switcher

- Alt+Tab opens the switcher in MRU order with the second-most-recent window selected.
- Hold Alt and press Tab or arrows to move. Focus does not change until commit.
- Release Alt to activate. Esc cancels and returns focus to the previous window.
- Clicking a tile activates it. Moving the pointer does not change the selection.
- The switcher is one row, max 6 tiles, horizontal scroll beyond 6.
- If only one window exists, it still opens and selects that window.

### 2.7 Virtual desktops

- Ctrl+Alt+Left/Right switches, clamped, no wrap.
- Focus on the destination desktop is the last focused window there; if none, desktop.
- A window being dragged cannot cross desktops; the desktop switch shortcut is ignored
  while a drag or resize is active.
- Switching is blocked while a menu or the launcher is open. Panels close first.

### 2.8 Transient input rules

- The launcher is modal. Global window and desktop shortcuts are suspended while it is
  open; Esc or Super closes it, and pointer input outside the panel closes it. The top
  bar remains usable and opening a top-bar menu closes the launcher first.
- Menus and the Alt-Tab switcher also suspend window and desktop shortcuts. Esc closes
  them; Alt+Tab controls the switcher itself.
- Control center and notification center are non-modal. Opening one moves keyboard focus
  into the panel; Esc closes it and returns focus to the previous control. Alt+Tab closes
  the panel and opens the switcher. A pointer click outside closes the panel and is
  consumed.
- Tooltips never take focus and never block input.
- A window drag or resize suspends the launcher, panels, menus and desktop switching
  until release or Esc.
- Esc with no transient open and no drag active is delivered to the focused app.

---

## 3. Pointer cursors

Procedural 16 x 16 bitmaps, 1 px white fill with a 1 px black outline so they remain
visible on any wallpaper and on light or dark surfaces. Scale with UI scale. Hotspots
are listed in logical pixels.

| Cursor | Hotspot | Used for |
|---|---|---|
| Arrow | 1, 1 | Default; titlebars, buttons, dock, tiles, panels |
| I-beam | 8, 8 | Text fields, selectable text, terminal, editor |
| Hand | 6, 2 | Hyperlinks and inline actions, never for buttons |
| Horizontal resize | 8, 8 | Window left/right edges, split handles |
| Vertical resize | 8, 8 | Window top/bottom edges |
| Diagonal resize NWSE | 8, 8 | Top-left and bottom-right corners |
| Diagonal resize NESW | 8, 8 | Top-right and bottom-left corners |

Rules:

- The cursor changes on entering a resize zone, before any press.
- While dragging or resizing, the cursor stays in the resize or arrow shape; it does not
  switch to a hand or a "move" cross.
- No wait or busy cursor in v1. Long operations must show in-app progress instead.
- No not-allowed cursor. Disabled controls use the arrow; they never use a hand.
- The cursor is never hidden automatically. It hides only when the pointer leaves the
  screen.
- Text selection uses the I-beam from press to release, including outside the field.

---

## 4. Component states

Baseline overlay rules:

| State | Dark | Light |
|---|---|---|
| Hover | white 6% over the component fill | black 5% over the component fill |
| Pressed | white 10% over the component fill | black 9% over the component fill |
| Selected | accent 16% over the fill | accent 14% over the fill |
| Disabled | whole component at 38% | whole component at 38% |
| Focus | 2 px ring, 2 px offset, `color.focus-ring` | same |

Pressed also applies a 0.98 scale to buttons and tiles, and 0.96 to checkboxes, for
120 ms. Cards, rows and menu items do not scale.

### 4.1 Buttons

| Variant | Rest | Hover | Pressed | Disabled |
|---|---|---|---|---|
| Primary (dark) | fill accent, label `#0B0D12` | black 8% over fill | black 14%, scale 0.98 | 38% |
| Primary (light) | fill strong accent, label `#FFFFFF` | black 8% | black 14%, scale 0.98 | 38% |
| Secondary | transparent, 1 px `border-control`, label `text` | hover overlay | pressed overlay | 38% |
| Ghost | transparent, label `text-secondary` | hover overlay, label `text` | pressed overlay | 38% |
| Danger (dark) | fill danger, label `#0B0D12` | black 8% | black 14% | 38% |
| Danger (light) | fill `#B42318`, label white | black 8% | black 14% | 38% |
| Destructive ghost | transparent, label `danger-text` | danger 10% fill | danger 16% fill | 38% |

- Height 28 or 32, radius 6, horizontal padding 12, label 13/18 600.
- Icon + label: 16 px icon, 8 px gap, icon inherits label color.
- Buttons never wrap. Min width 64 for text buttons, 28 for icon-only.
- Loading state is not specified in v1; do not invent a spinner unless it is added here.

### 4.2 Icon buttons

- 28 x 28, radius 6, icon 16, icon `text-secondary`.
- Hover: overlay + icon `text`.
- Pressed: overlay + icon `text` + scale 0.98.
- Selected: accent 16% fill + accent icon (for example an active view mode).
- Disabled: 38%, no hover, not focusable.
- Tooltip required. The accessible label is the tooltip text.

### 4.3 Toggles

- Track 36 x 20, radius pill; thumb 16 circle, 2 px inset.
- Off: track `#2A3140` dark / `#D8DEE8` light; thumb `#FFFFFF` dark / `#12151B` light.
- On: track accent dark / strong accent light; thumb `#FFFFFF` dark / `#12151B` light.
- Hover: 2 px accent at 20% outside the track.
- Pressed: thumb stretches to 20 x 16 for 120 ms, then settles. Reduce motion: no stretch.
- Focus: ring around the track at 2 px offset.
- Disabled: 38%, no hover; state remains readable.
- Hit target 44 x 28 minimum. The label is clickable and toggles too.

### 4.4 Checkbox and radio

- Visual 16 x 16, radius 6 (checkbox) or circle (radio), 1 px `border-control`, fill
  `surface`.
- Hover: border `text-secondary` and a 6% overlay fill.
- Pressed: scale 0.96.
- Checked: accent fill, 10 px check in ink dark / white light. Radio: accent fill with a
  6 px ink or white dot.
- Indeterminate checkbox: 8 x 2 ink or white bar.
- Focus: ring around the 28 x 28 hit box, not the 16 px box.
- Disabled: 38%, no hover. The label does not toggle.
- Hit target 28 x 28; label gap 8.

### 4.5 Text and search fields

- Text field: 32 high, radius 10, 1 px `border-control`, fill `surface`, padding 10,
  text 13/18, placeholder `text-secondary`.
- Hover: border `text-secondary`.
- Focus: 2 px focus ring; border stays `border-control` (the ring is the state).
- Error: border `danger`, error text below (section 6.1); focus ring remains accent.
- Disabled: fill hover overlay at 4%, text `text-tertiary`, no hover, not focusable.
- Search field: 28 high, radius 6, 1 px `border-control`, fill `surface`, 16 px search
  icon at 8, text starts at 32. Hover border `text-secondary`. Clear button 16 at the
  right when non-empty.
- Selection: accent 30%, text unchanged.
- Caret: 1 px accent, 530 ms blink; solid while typing and under reduce motion.

### 4.6 Sliders

- Track 4 px `hairline`, filled portion accent, knob 14 circle `text` with 1 px
  `rgba(0,0,0,0.20)` ring.
- Hover: 2 px accent at 20% around the knob; no size change.
- Pressed: knob scale 0.92; dragging continues outside the track.
- Focus: 2 px ring around the knob at 4 px offset.
- Keyboard: arrows change by 1 step, Shift+arrows by 10%, Home/End to min/max.
- Disabled: 38%, not focusable.
- Hit height 28; the whole track is clickable and jumps to the value.

### 4.7 Lists and tables

- Row 36 high. Hover: overlay + radius 6 inset 4 horizontal. Selected: accent 16% +
  2 px accent bar on the leading edge, radius 6. Pressed: pressed overlay.
- Focus: 2 px ring inset 2 when the row itself is the focus target. In a roving list,
  the focused row shows the selected fill, not a ring, and the list shows a focus ring
  around the viewport only when the list has focus.
- Disabled row: 38%.
- Table header 28, 11/14 500 `text-secondary`, no hover. Numeric columns right aligned
  with tabular digits. Sort indicator 14 px chevron.
- Double-click a row: app-defined primary action (open the file, note, event).
- Ctrl+click toggles selection; Shift+click extends. Multi-select is app-defined.
- Empty list: section 5.

### 4.8 Tabs and segmented controls

- Tab bar 28 high inside the 44 toolbar. Tab 28 high, padding 12, radius 6,
  `text-secondary`, 13/18 600.
- Hover: overlay. Active: `text`, 2 px accent underline at the bottom edge inset 8.
  Pressed: overlay.
- Disabled: 38%.
- Focus: inset 2 px ring.
- Segmented control: container 28, radius 6, fill overlay 4%, padding 2. Segment 24 high
  (hit 28), radius 6. Selected: `surface-elevated` + 1 px hairline + `text`; unselected
  `text-secondary`. Arrow keys move selection within the group.

### 4.9 Menus, top bar and window controls

- Menu item: 28 high, radius 6, hover overlay. Disabled 38%, no hover. No ring inside
  the menu; the keyboard highlight uses the same hover fill. Section 12.1 of
  DESKTOP_SPEC.md.
- Top bar menu title: 28 high hit, 24 high pill radius 6 inset 2. Hover overlay, pressed
  overlay, open (menu visible) selected fill.
- Window control: 12 circle, rest neutral fill with a 1 px `border-control` edge,
  hover semantic fill (amber / green / danger), glyph appears, pressed black 14% over
  the semantic fill, focus ring at 0 offset, disabled only when the window cannot
  minimize/close (not in v1).
- Dock item: hover magnifies (DESKTOP_SPEC.md 5.2), pressed scale 0.96, focused shows a
  12 x 3 accent pill, running shows 4 x 3 secondary dot. Disabled does not exist.
- Launcher tile: hover 6%, pressed 10% + scale 0.97, focus inset 2 ring, no disabled.
- Notification card: hover 4%, pressed 9%, focus inset 2 ring. Dismiss button is a
  28 x 28 icon button revealed on hover or focus.
- Scrollbar: 8 px track transparent, 4 px thumb `color.border-control`, hover to
  `text-secondary` and thumb grows to 8, drag `text`. Not keyboard focusable;
  PageUp/PageDown and arrows scroll.

### 4.10 Focus and disabled rules for all components

- Focus ring is never removed to "clean up" a design. It may be inset only when clipped.
- Disabled components are removed from the tab order. The reason must be visible in
  helper text when it is not obvious.
- Disabled components keep their text label at 38% total opacity; do not recolor to
  `text-tertiary` on top of an already dimmed component.
- Hover and pressed states never change geometry except the 0.98 scale and the toggle
  thumb stretch. No layout shift.
- A component under the pointer must show hover within one frame (<= 16 ms).
- Pressed state applies on pointer down, releases on pointer up anywhere, and cancels if
  the pointer leaves the component by more than 8 px before release.

---

## 5. Empty states

Pattern:

- Centered in the content area. Max width 320. Vertical padding 40.
- Icon 32, `text-secondary` at 70%, 16 px below.
- Title 15/20 600, `text`.
- Body 13/18 `text-secondary`, max 2 lines, 8 px below the title.
- Optional primary action 28 px high, 16 px below the body. At most one action.
- No illustration, no mascot, no marketing copy.
- The empty state must not hide the toolbar or the ability to act.

| Context | Icon | Title | Body | Action |
|---|---|---|---|---|
| Files, empty folder | folder | This folder is empty | Files you add appear here. | New folder |
| Files, no results | search | No matches | Try a shorter name or a different folder. | Clear search |
| Text, no document | file | No document open | Create a document or open one from Files. | New document |
| Notes, no note selected | file | No note selected | Choose a note or create a new one. | New note |
| Notes, no notes | file | No notes yet | Notes you create appear here. | New note |
| Photos, empty library | image | No photos yet | Add images to Pictures in Files. | None |
| Notifications, empty | bell | No notifications | System and app alerts appear here. | None |
| Launcher, no results | search | No apps found | "<query>" | None |
| Monitor, unavailable | chart | Metrics unavailable | The kernel did not expose counters. | Retry |
| Calendar, no events today | calendar | Nothing scheduled | Events you add appear here. | None |

Terminal has no empty state: an empty terminal is a prompt. Calculator has no empty
state. Settings has no empty state.

---

## 6. Error presentation

### 6.1 Inline field error

- Field border becomes 1 px `color.danger-text` (dark `#F87171`, light `#B42318`).
- 12 px `color.danger-text` icon and 11/14 500 `danger-text` message, 4 px below the
  field.
- Reserve 16 px under fields in validated forms so the message does not shift layout.
- Multiple fields: show one message per field, not a summary.
- On submit, focus the first invalid field and scroll it into view.
- The message names the fix: "Use at least 8 characters", not "Invalid input".

### 6.2 Banner

- Full content width, radius 10, padding 12, 1 px `danger-text` at 40%, fill danger 8%.
- Icon 16 `danger-text` at 12, title 13/18 600, body 12/16 `text-secondary`, dismiss
  28 x 28 at right.
- Used for async or form-level failures. Persists until dismissed or retried.
- Only one banner at a time per view; a new one replaces the old.

### 6.3 Toast

- 360 wide, min 72 high, `danger-text` icon, title "Could not save changes", body one
  line.
- Auto-dismiss 5 s with hover/focus pause and persistence in the notification center.
- Clicking opens the source view. Never contains the only copy of the error.

### 6.4 Dialog

- 360 wide (simple) or 440 wide (with detail). Radius 14, `surface-elevated`, panel
  border, `elev.3`.
- Scrim `color.scrim` over the owning window only, not the whole desktop.
- Title 15/20 600, body 13/18, actions right aligned, 8 px gap, 32 px high.
- Destructive action uses the danger button; Cancel is the default focus and is
  reachable with Esc.
- Focus is trapped. Closing returns focus to the invoker.
- Used only for irreversible actions or unsaved-data confirmation. Not for routine
  information.

### 6.5 Error copy rules

- Order: what happened, why (if known), what to do next.
- No "Error:" prefix. No error codes as the message; put a code in secondary text if it
  helps support.
- No blame ("You entered invalid data") and no humor.
- Never use color alone: every error has an icon and text.
- A failed background operation must leave the previous state intact and say so.
- Do not claim data was saved unless it was committed to storage.

Example set:

| Situation | Message |
|---|---|
| Save failed | "Could not save. The disk is full. Free space in Files, then try again." |
| File missing | "This file no longer exists. It may have been moved or deleted." |
| Invalid name | "Names cannot contain / or :. Choose another name." |
| Monitor unavailable | "Metrics unavailable. The kernel did not expose counters." |

---

## 7. Notifications

### 7.1 Timing and stacking

- Auto-dismiss after 5000 ms.
- Hovering anywhere in the toast stack pauses all timers. Keyboard focus inside a toast
  pauses that toast. Resume with the remaining time, never reset to 5 s.
- Max 4 toasts visible, 8 px apart, newest top. Additional notifications queue.
- Dismissing a toast does not move the toast above it; the stack below moves up 200 ms.
- The timer is real time; it is not affected by animation speed, reduce motion or the
  FPS cap.
- If the notification center is open, no toast is shown; the item appears in the center.
- Do not disturb suppresses toasts only; items are still recorded.
- There is no sticky or critical toast in v1.

### 7.2 Actions

- Click the toast: open the source app, focus the relevant window, dismiss the toast.
- Dismiss button: 28 x 28, visible on hover or keyboard focus, always present in the
  hit tree for the toast's 5 s lifetime.
- No reply, archive, snooze or inline action buttons in v1.
- Notifications are read-only after dismissal except for opening the source.

### 7.3 Content rules

- Title <= 40 characters, body <= 120, one line each in the toast, two lines in the
  center for the body.
- No marketing, no tips, no "Did you know", no badge counts as the only content.
- One notification per event. Group repeated events ("3 files copied") rather than
  stacking three.
- Never interrupt a drag, resize, text selection, or menu with a toast that takes focus.
  Toasts never take focus.

---

## 8. Accessibility behavior

### 8.1 Keyboard access

- Every action reachable by pointer is reachable by keyboard, except direct edge
  resizing, which has snap and maximize equivalents.
- Tab order follows visual order. Composite widgets are one tab stop with arrow-key
  navigation inside.
- Focus is always visible and never clipped. Scroll the focused item into view with an
  8 px margin.
- Dialogs and the launcher trap focus until dismissed; Esc always exits and returns
  focus to the invoker.
- Menus use a roving highlight; opening a menu moves focus to the first enabled item.
  Esc closes and returns focus to the menu title.
- F10 focuses the menu bar. Shift+F10 opens the context menu for the focused element.
- No keyboard trap. No shortcut requires simultaneous three-key chords except
  Ctrl+Alt+arrows, which is documented in the Settings keyboard reference.

### 8.2 Focus policy

- Show the ring for keyboard navigation, for text fields, and after programmatic focus
  when the last input was keyboard.
- Pointer presses on buttons may suppress the ring until the next key press.
- Hover never moves focus.
- A focused window's focused control keeps its ring when the window is active. When the
  window loses focus, the ring is hidden but the control remains the window's focus
  target when refocused.

### 8.3 Target size and tolerance

- Minimum 28 x 28 logical px, 8 px separation where layout permits.
- Window controls are 20 x 28 with menu and keyboard equivalents.
- Resize edges are 6 px but have a 2 px tolerance outside the frame and equivalent
  keyboard actions.
- Text links are inline and exempt.
- Press tolerance: release within 8 px of the component bounds still activates; beyond
  that cancels.

### 8.4 Contrast and color

- All active text >= 4.5:1; large text >= 3:1.
- Control boundaries, toggle states, focus rings and selection >= 3:1.
- `text-tertiary` is not used for active text.
- State is never color-only (DESIGN_SYSTEM.md 10.3).
- Selection, error, warning and success always include a non-color cue.

### 8.5 Motion

- Respect `reduce_motion`: no scale, slide, magnification or caret blink; opacity fades
  capped at 120 ms.
- Respect `animation_speed` 0..2; 0 disables animation entirely.
- No flashing above 3 Hz.
- Nothing moves continuously in the periphery. The only idle animation is the text
  caret, and it stops under reduce motion.

### 8.6 Timing

- Toasts are the only timed UI. They pause on hover/focus and persist in the center, so
  information is recoverable.
- No other interface times out. Menus, panels and dialogs stay open until dismissed.
- Auto-repeat and double-click timings follow section 1.3.

### 8.7 Text

- Text selection has a visible 30% accent fill and keeps text contrast.
- The caret is 1 px and blinks at 530 ms; it is solid while typing and under reduce
  motion.
- Text is not rendered below 11 px.
- UI scale 125/150/200 is supported; hit targets scale with it.
- Long text wraps; it is never clipped horizontally without an ellipsis.

### 8.8 Unsupported in v1

Aster OS has no screen reader bridge, no voice control, no captions system, no
high-contrast theme and no reduce-transparency mode. Do not state or imply that these
exist. Do not use "accessibility" as a label for a feature that only changes color.

---

## 9. State transition summary

| Event | Immediate result | Animated result |
|---|---|---|
| Click inactive window | Focus, raise, deliver click | None |
| Press titlebar, move 3 px | Drag begins | Window follows pointer 1:1 |
| Drag to edge, dwell 120 ms | Snap preview appears | 120 ms preview fade |
| Release in snap zone | Window snaps | 200 ms rect interpolation |
| Double-click titlebar | Maximize/restore commits | 200 ms rect interpolation |
| Alt+Tab press | Switcher opens | 120 ms fade |
| Release Alt | Selected window focused, switcher closes | 120 ms fade |
| Super tap | Launcher opens, search focused | 120 ms panel + 200 ms scrim |
| Esc on launcher | Launcher closes, focus returns | 120 ms fade |
| Toast arrives | Toast added, timer starts | 200 ms enter |
| Hover toast | Timer pauses | None |
| 5 s elapse | Toast removed, stored in center | 120 ms exit |
| Ctrl+Alt+Right | Desktop index changes | 320 ms slide/fade |
| Reduce motion on | All transforms become instant | Opacity <= 120 ms |
