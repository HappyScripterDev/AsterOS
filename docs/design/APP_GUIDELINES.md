# Aster OS App Guidelines

Status: normative v1. All built-in apps use the shared shell and tokens in
DESIGN_SYSTEM.md, DESKTOP_SPEC.md and INTERACTION.md. Diagrams are not to scale.

Built-in apps in v1:

1. Files
2. Terminal
3. Text
4. Notes
5. Calendar
6. Photos
7. Calculator
8. Monitor
9. Settings

---


> **Shipped app shell (2026-11-20).** All apps are light-only. The Settings
> sidebar is 186 px wide with coloured section tiles; Files uses a Finder-style
> sidebar and file icons; window titles are centred with traffic lights on the
> left. Terminal and Text use the monospaced face; the rest use Inter.


## 1. Shared shell

Every app window is a normal window: 28 px titlebar, radius 14, opaque
`color.surface` body, 1 px border, `elev.4` when focused. Apps do not draw their own
chrome and do not change titlebar colors.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                     App title                            | 28
+----------------------------------------------------------------------+
| TOOLBAR 44: [controls 28] .................. [search 220x28] [action]| 44
+----------------+-----------------------------------------------------+
| SIDEBAR 200    | CONTENT                                             |
| canvas         | surface, padding 20                                 |
| nav rows 32    | list rows 36, fields 32, buttons 28                 |
|                |                                                     |
+----------------+-----------------------------------------------------+
```

### 1.1 Frame budget

| Region | Value |
|---|---|
| Window body | `width` x (`height` - 28) |
| Toolbar | 44 high, full width, bottom hairline |
| Content below toolbar | `height - 72` |
| Sidebar | 200 wide, full height below the toolbar |
| Content column with sidebar | `width - 200` |
| Content column without sidebar | `width` |
| Content padding | 20 on all sides |
| Usable content with sidebar at 880 wide | 640 |
| Usable content without sidebar at 880 wide | 840 |

### 1.2 Toolbar

- Height 44, background `surface`, bottom 1 px hairline.
- Horizontal padding 12. Vertical padding 8, so 28 px controls sit centered.
- Left: navigation and primary controls, 8 px gaps. A 28 x 28 icon button is the default.
- Right: search field (220 x 28, radius 6) and one primary action (28 high).
- No app name or window title in the toolbar; the titlebar owns it.
- No second toolbar row. If controls do not fit, overflow into a 28 x 28 "more" menu.
- Toolbar controls are 28 high, never 24. Minimum target applies.
- Toolbar buttons show tooltips. Text buttons use 13/18 600, padding 10.

### 1.3 Sidebar

- Width 200, background `color.canvas` dark / `color.surface-elevated` light.
- Right border 1 px hairline.
- Top padding 12, bottom padding 12, horizontal 8 inside the 200, so rows are 184 wide.
- Section label: 11/14 500 `text-secondary`, 24 high, padding 12 left.
- Navigation row: 32 high, radius 6, icon 16, gap 8, padding 8, label 13/18.
- Hover: `state.hover-overlay`. Selected: `state.selected-fill` + 2 px accent bar on
  the leading edge. Pressed: pressed overlay. Focus: inset 2 px ring.
- Navigation rows are 32, not 36. They are not content list rows; this is deliberate
  and consistent across apps.
- No collapsible sidebar in v1; no drag-to-resize sidebar.

### 1.4 Content

- Padding 20 on all sides. Content scrolls inside the padded area; scrollbar sits in the
  8 px gutter, outside the 20 px inset.
- Background `color.surface`; sidebar is the only tonal split.
- Cards: `surface-elevated`, radius 14, 1 px `rgba(255,255,255,0.06)` dark /
  `rgba(18,22,31,0.08)` light, no shadow.
- Section header: 15/20 600, 24 high, 16 px above the first row, 8 px below.
- Page title (large settings/empty pages): 22/28 600, 28 high.
- Body copy max width 680. Two-column forms: label column 200, control column 320,
  remaining space empty. Do not stretch controls edge to edge. When the content column
  is narrower than 520, stack the label above the control; the control keeps its 320
  maximum.
- Dialogs follow INTERACTION.md 6.4.

### 1.5 Lists and tables

- Default row 36. Row = 8 px horizontal inset, 8 px padding, 13/18 label.
- Hover overlay, selected accent 16% + 2 px bar, pressed overlay, focus inset ring.
- Divider: only between groups, 1 px hairline; not between every row.
- Table header 28, 11/14 500 `text-secondary`.
- Columns: name flexes, size 80 right aligned, modified 140 right aligned. Numeric cells
  use tabular digits.
- Double-click row = open. Enter = open. Space = preview if the app defines one.
- Multi-select: Ctrl+click toggles, Shift+click extends, Ctrl+A selects all.
- Empty list follows INTERACTION.md 5.

### 1.6 Controls

| Control | Size |
|---|---|
| Icon button | 28 x 28, icon 16, radius 6 |
| Text button | 28 high, padding 10, radius 6, 13/18 600 |
| Primary action | 28 high, same metrics, accent fill |
| Search field | 220 x 28, radius 6, icon 14 |
| Form field | 32 high, radius 10, text 13/18, padding 10 |
| Toggle | 36 x 20 track, 44 x 28 hit |
| Checkbox / radio | 16 visual, 28 x 28 hit |
| Slider | 4 track, 14 knob, 28 hit |
| Tab | 28 high, padding 12, radius 6 |
| Table header | 28 |
| Setting row | 36 without description, 48 with description |
| Scrollbar | 8 track, 4 thumb |

- All controls use the states in INTERACTION.md 4.
- Forms validate on submit, then on change after the first error. Errors per
  INTERACTION.md 6.1.
- Primary action is on the right of the toolbar. Only one primary action per view.
- Destructive actions are never the default focus and never adjacent to a common action
  without an 8 px gap.

### 1.7 Menus

All apps provide Window and Help. App-specific menus must only contain implemented
actions. Minimum:

| App | Menus beyond Window/Help |
|---|---|
| Files | File (New Window, New Folder, Close Window), Edit (Cut, Copy, Paste, Rename, Delete, Select All), View (Refresh) |
| Terminal | File (New Window, New Tab, Close Tab, Close Window), Edit (Copy, Paste, Select All, Clear Scrollback) |
| Text | File (New, Open, Save, Close), Edit (Undo, Redo, Cut, Copy, Paste, Select All) |
| Notes | File (New Note, Delete Note), Edit (Undo, Redo, Cut, Copy, Paste, Select All) |
| Calendar | File (New Event, Close Window), View (Today, Previous Month, Next Month) |
| Photos | View (All Photos, Recents) |
| Calculator | Edit (Copy Result) |
| Monitor | View (Refresh) |
| Settings | None beyond Window/Help |

No app menu may contain Store, Installer, Update, Browser, network, account, cloud,
purchase or "coming soon" items.

### 1.8 Window defaults

| App | Default | Minimum | Single instance | Sidebar |
|---|---|---|---|---|
| Files | 880 x 560 | 640 x 400 | No | Yes |
| Terminal | 760 x 480 | 480 x 300 | No | No |
| Text | 720 x 560 | 480 x 360 | No | No |
| Notes | 900 x 600 | 640 x 400 | Yes | Yes |
| Calendar | 960 x 640 | 720 x 480 | Yes | Yes |
| Photos | 960 x 640 | 720 x 480 | Yes | Yes |
| Calculator | 360 x 480 | 360 x 480 | Yes | No |
| Monitor | 880 x 560 | 640 x 400 | Yes | Yes |
| Settings | 820 x 600 | 640 x 420 | Yes | Yes |

All sizes are logical and clamp to the work area. The global 360 x 240 minimum still
applies; the per-app minimum wins when larger.

---

## 2. Files

Default 880 x 560. Sidebar 200. Toolbar: back, forward, up, breadcrumb, search, New
folder.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                        Files                             |
+----------------------------------------------------------------------+
| [<] [>] [^]  Home / Documents          [search 200]     [+ Folder]   |
+----------------+-----------------------------------------------------+
| Places         | Name                    Size        Modified        |
|  Home          | Report.txt              12 KB       Nov 20 14:02    |
|  Documents     | Budget.csv              4 KB        Nov 19 09:10    |
|  Downloads     | Notes.md                2 KB        Nov 18 21:44    |
|  Pictures      |                                                     |
|                |                                                     |
+----------------+-----------------------------------------------------+
```

- Breadcrumb: 13/18, segments clickable, 16 px chevrons `text-secondary`, current
  segment `text`; overflow collapses to "..." with a menu.
- Back/forward/up are 28 x 28 icon buttons; disabled at history ends.
- New folder opens an inline name field at the top of the list, 36 high, selects the
  default name "Untitled folder". Enter commits, Esc cancels.
- Rename uses the same inline field. Delete is permanent in v1 and always asks for
  confirmation; there is no Trash folder.
- List only in v1. No grid view toggle, no column resizing, no drag-and-drop.
- Sort by clicking a header. Size and Modified are right aligned with tabular digits.
- Search filters the current folder; results replace the list; Esc clears.
- Status: item count in the toolbar right, 11/14 secondary, before the search field.
- Empty folder: folder icon, "This folder is empty", "Files you add appear here.",
  "New folder" action.
- No file preview pane in v1.

Keyboard: Ctrl+F search, Ctrl+A select all, Delete permanently deletes with
confirmation, Enter open, Backspace up one level, Alt+Left/Right history.

---

## 3. Terminal

Default 760 x 480. No sidebar. Toolbar: tab strip, new tab, clear scrollback.

```
+------------------------------------------------------------------+
| (o) (o) (o)                    Terminal                          |
+------------------------------------------------------------------+
| [Terminal 1] [Terminal 2] [+]                      [clear]       |
+------------------------------------------------------------------+
| aster ~ $ ls -la                                                  |
| total 12                                                          |
| drwxr-xr-x  3 aster aster 4096 Nov 20 14:02 .                     |
| -rw-r--r--  1 aster aster  412 Nov 20 14:02 notes.txt             |
| aster ~ $ _                                                       |
|                                                                   |
+------------------------------------------------------------------+
```

- Background `color.canvas`, padding 20, no card.
- Grid: cell 7.8 x 18 at 13 px, 92 columns x 20 rows at the default size, recomputed on
  resize. Tab stops every 8 cells.
- **Known limitation:** Inter is not monospaced. Glyphs are centered in fixed cells,
  which aligns columns and prompts approximately. Full-screen TUI applications,
  box-drawing characters, wide CJK glyphs and arbitrary Unicode are not supported in
  v1. Do not claim a VT100/xterm-compatible terminal.
- Prompt: user 13/18 600 accent, path 13/18 secondary, `$` secondary, command text.
- Output 13/18 `text`; stderr `danger-text`; success notices `success-text`.
- Cursor: block 1 cell, 80% accent, blink per INTERACTION.md 8.7. While a command runs,
  the cursor is solid and a 2 px accent line sits at the top of the content area? No:
  do not add a progress bar. Use a solid cursor and the last line "running..." in
  secondary.
- Selection: drag with I-beam, double-click word, triple-click line. Ctrl+Shift+C copies
  and Ctrl+Shift+V pastes (in addition to the Edit menu), because Ctrl+C must remain the
  interrupt signal.
- Scrollback 1000 lines. Wheel scrolls 48 px. Scrollbar 8 px.
- Tabs: 28 high, max width 160, radius 6; active `surface-elevated` + 2 px accent
  underline; close X 16 on the active tab; + button 28.
- Clear scrollback: toolbar icon, clears the visible buffer after a 120 ms fade. No
  confirmation.
- No split panes, no search-in-scrollback, no font size control, no profiles.
- Empty terminal is a prompt, not an empty state.

---

## 4. Text

Default 720 x 560. No sidebar. Toolbar: New, Open, Save, word count.

```
+------------------------------------------------------------------+
| (o) (o) (o)                      Text                            |
+------------------------------------------------------------------+
| [New] [Open] [Save]                             142 words        |
+------------------------------------------------------------------+
|                                                                  |
|  Untitled                                                        |
|                                                                  |
|  The quick brown fox jumps over the lazy dog.                    |
|  A second paragraph starts here and wraps at 680 px.             |
|  |                                                               |
|                                                                  |
+------------------------------------------------------------------+
```

- Editor starts at x=20, y=20, width 680 max, no card, no border.
- Title: 22/28 600, one line, placeholder "Untitled".
- Body: 15/24 reading style (see DESIGN_SYSTEM.md 3.2), `text`, max width 680.
- Caret and selection per INTERACTION.md.
- Toolbar: New 28 text button, Open/Save 28 icon+label, word count 11/14 secondary at
  the right, 8 px before the edge.
- No rich text, no markdown preview, no line numbers, no minimap.
- Save: overwrite in place. Save As is not in v1. If no path, Save opens a 360 px dialog
  with a name field and Cancel/Save.
- Unsaved changes: closing shows the in-window modal "Save changes?" with Save, Discard,
  Cancel. Cancel is default.
- Open: 440 px dialog listing files from the Files app's current folder model; no
  fabricated cloud locations.
- Empty document: blank editor with caret; no empty-state card.
- Scrollbar 8 px on the right of the content area.

Keyboard: standard text editing, Ctrl+S save, Ctrl+O open, Ctrl+N new window, Ctrl+W
close document.

---

## 5. Notes

Default 900 x 600. Sidebar 200. Toolbar: New note, Delete, search.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                         Notes                            |
+----------------------------------------------------------------------+
| [New note] [Delete]                              [search 220]        |
+----------------+-----------------------------------------------------+
| Today          |  Meeting notes                     Nov 20, 2026    |
|  Meeting notes |                                                     |
|  Groceries     |  Discussion points for the desktop spec.            |
|  Ideas         |  - Dock height                                     |
|                |  - Snap thresholds                                  |
|                |                                                     |
+----------------+-----------------------------------------------------+
```

- Sidebar: 200, list rows 36. Row: title 13/18 600, date 11/14 secondary right, no body
  preview (36 px cannot hold two lines). Selected row accent fill + 2 px bar.
- Toolbar: New note 28 text button, Delete 28 icon button (disabled when none selected),
  search 220 x 28.
- Editor: title 22/28 600, date 11/14 secondary, body 15/24 reading, max 680, padding 20.
- Autosave on blur and 800 ms after the last keystroke. Show "Saved" 11/14 secondary in
  the toolbar right for 2 s. Never claim saved before the write completes.
- Delete: confirmation dialog "Delete note?" with Delete (danger) and Cancel. No undo for
  note deletion in v1; do not offer one.
- Empty note list: "No notes yet" + New note.
- No note selected: "No note selected" + New note.
- No folders, tags, pinning, or rich text in v1.
- No drag-and-drop of notes.

---

## 6. Calendar

Default 960 x 640. Sidebar 200. Toolbar: previous, month label, next, Today.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                       Calendar                           |
+----------------------------------------------------------------------+
| [<]  November 2026  [>]  [Today]                                     |
+----------------+-----------------------------------------------------+
| Mini month     | Mon   Tue   Wed   Thu   Fri   Sat   Sun            |
|  M T W T F S S |  26    27    28    29    30    31     1             |
| Calendars      |  2     3     4     5     6     7     8             |
|  Personal      |  [12:00 Design review]                              |
|                |  ...                                                |
+----------------+-----------------------------------------------------+
```

- Sidebar: 200. Mini month 7 x 6 cells of 24 px, inside 184 px: 7*24 = 168, centered
  with 8 px margins. Weekday initials 11/14 secondary. Today accent fill, ink label.
- Calendars section: "Personal" with a 8 px accent dot; only one calendar in v1. Do not
  list calendars that do not exist.
- Toolbar: prev/next 28 icon buttons, month label 15/20 600, Today 28 text button.
  Right side: no view toggle (month only).
- Content: weekday header 24, then 6 week rows. Cell height = `(contentH - 24) / 6`,
  about 84 at the default size. Grid 1 px hairline. 7 equal columns, fractional widths
  allowed (about 102.86 at 960).
- Day number 11/14 500 secondary, top left of the cell, 6 px inset. Today: 20 px circle
  accent fill, label `#0B0D12` dark / `#FFFFFF` light.
- Event chip: height 16, radius 6, accent 16% fill, 2 px accent left bar, label 11/14
  500 `text`, max width cell - 8, ellipsis. Max 2 chips per cell, then "+N" 11/14
  secondary. Below 72 px cell height, show 1 chip plus "+N".
- Double-click a day or "+N": opens a 360 px "New event" dialog: title field, date,
  start time, end time, Save/Cancel. No recurrence, reminders, attendees or locations.
- No week/day/agenda view in v1. No drag to move events. No external calendars.
- Empty cell shows nothing; there is no calendar-wide empty state. If a day has no
  events, no placeholder text.

---

## 7. Photos

Default 960 x 640. Sidebar 200. Toolbar: title, count, search.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                        Photos                            |
+----------------------------------------------------------------------+
| All photos  12                                      [search 220]     |
+----------------+-----------------------------------------------------+
| Library        | [ 168x112 ] [ 168x112 ] [ 168x112 ] [ 168x112 ]    |
|  All photos    | [ 168x112 ] [ 168x112 ] [ 168x112 ] [ 168x112 ]    |
|  Recents       |                                                     |
| Albums         |                                                     |
+----------------+-----------------------------------------------------+
```

- Sidebar: Library (All photos, Recents), Albums. Rows 32. Empty albums are not listed.
- Toolbar: "All photos" 15/20 600, count 11/14 secondary, search 220 x 28.
- Grid: 4 columns at the default 960 width, thumb 168 x 112 (3:2), gap 12, radius 10, at
  x=20. Total block 708, left aligned; leftover width stays empty. No masonry. Column
  count is `clamp(floor((contentW + 12) / 180), 2, 6)`, where contentW is the width after
  the sidebar and 20 px padding; the minimum 720 window uses 2 columns.
- Thumbnail hover: 6% overlay, 16 px expand glyph bottom right. Click selects, double
  click opens the viewer; Enter opens.
- Viewer: inside the window, scrim `rgba(5,7,11,0.86)`, image fit with 20 px margin,
  close 28 x 28 top right, previous/next 28 x 28 centered vertically, filename 11/14
  white, counter 11/14 white at top left. Esc closes, Left/Right navigate.
- Empty library: image icon, "No photos yet", "Add images to Pictures in Files." No
  Import button; import is not implemented in v1.
- Search matches filename, not content. No albums, faces, favorites, editing, slideshow
  or sharing in v1.
- Do not use the current wallpaper as a stock photo.

---

## 8. Calculator

Default and minimum 360 x 480. No sidebar. Toolbar: mode label, copy result.

```
+----------------------------------+
| (o) (o) (o)      Calculator      |
+----------------------------------+
| Standard                  [copy] |
+----------------------------------+
|                                  |
|                      1,234.56    |
|  12 x 102.88 =                   |
+----------------------------------+
| [ C ] [ +/- ] [ % ] [ / ]        |
| [ 7 ] [ 8 ]   [ 9 ] [ x ]        |
| [ 4 ] [ 5 ]   [ 6 ] [ - ]        |
| [ 1 ] [ 2 ]   [ 3 ] [ + ]        |
| [    0    ]   [ . ] [ = ]        |
+----------------------------------+
```

- Body 480 - 28 - 44 = 408. Padding 20 leaves 368. Display 88, gap 8, 5 rows of 48 with
  4 gaps of 8 = 272. Total 368 exactly.
- Display: right aligned, padding 12, expression 12/16 secondary above, result 28/34
  600 tabular below. Max 12 digits; then switch to headline 22/28; then ellipsis. No
  scientific notation in v1.
- Buttons: 4 columns, 74 x 48, gap 8, radius 10. Digit buttons `surface-elevated`,
  label 15/20 600. Operator buttons `surface-elevated`, accent label. Equals: accent
  fill, ink dark / white light. C: `danger-text`. Zero spans 2 columns (156).
- Operators highlight (accent 16%) while waiting for the second operand.
- Toolbar: "Standard" 13/18 600 left; copy icon 28 right, copies the result as plain
  text, tooltip "Copy result".
- Keyboard: 0-9, `.`, `+ - * /`, Enter/=, Esc/C clear, Backspace delete, `%`.
- No history, memory keys, scientific mode, unit conversion or programmer mode.
- Error: "Cannot divide by zero" in the expression line, result shows "Error"; entering
  a digit clears it.
- Resizing larger stretches the display and buttons; the 4 x 5 structure stays.

---

## 9. Monitor

Default 880 x 560. Sidebar 200. Toolbar: refresh, interval.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                       Monitor                            |
+----------------------------------------------------------------------+
| [refresh]  1 s                                            [pause]    |
+----------------+-----------------------------------------------------+
| Overview       | +---------+ +---------+ +---------+                |
| CPU            | | CPU 12% | | MEM 38% | | PROC 41 |                |
| Memory         | +---------+ +---------+ +---------+                |
| Processes      | [ 640 x 160 line chart ]                            |
|                | Name          PID     CPU      Memory              |
|                | compositor    12      8%       42 MB               |
+----------------+-----------------------------------------------------+
```

- Sidebar: Overview, CPU, Memory, Processes. Rows 32.
- Toolbar: refresh 28 icon, "1 s" 11/14 secondary, spacer, Pause/Resume 28 text button.
  Refresh interval fixed at 1 s; no dropdown.
- Overview cards: 200 x 88, gap 12. Three across at 640 content width (624); at widths
  below 640, wrap to 2-up, and below 420 to 1-up. Label 11/14 secondary, value
  22/28 600 tabular. Cards: CPU %, Memory used %, Process count.
- Chart: width = content width (640 at the default size), height 160, 1 px hairline grid
  at 0/25/50/75/100%, 2 px accent polyline, 60 samples (60 s). No fill, no gradient.
  Y max 100%. Time axis labels 11/14 at 0 s and 60 s only.
- CPU page: the same 640 x 160 chart plus one 36 px row per core, label "Core N", load
  percentage right aligned. No per-core chart.
- Memory page: three 200 x 88 cards (used, free, total) and a 640 px list of the five
  largest processes by memory, 36 px rows. No swap card unless the kernel reports swap.
- Processes page: the full process table, same columns, scrolls independently.
- Overview combines the three cards, the chart and the top five processes.
- Process table: header 28, rows 36, columns Name flex, PID 64 right, CPU 64 right,
  Memory 80 right, tabular digits. Click header to sort. Scrolls; header stays.
- If a counter is unavailable: "Metrics unavailable" empty state with Retry, not zeros.
- Values must come from the kernel; never fabricate activity or smooth it into a fake
  curve. A flat line is a valid result.
- No kill, priority, affinity, temperature, GPU, disk or network panes in v1.
- No persistent process details pane. Double-click a process opens a 440 px dialog with
  PID, state, CPU and memory; no terminate button.

---

## 10. Settings

Default 820 x 600. Sidebar 200. Content max 640.

```
+----------------------------------------------------------------------+
| (o) (o) (o)                       Settings                           |
+----------------------------------------------------------------------+
| [search 220]                                                          |
+----------------+-----------------------------------------------------+
| Appearance   > | Appearance                                          |
| Desktop        |                                                     |
| Notifications  | Theme              [ Dark ] [ Light ]               |
| Keyboard       | Accent             (o)(o)(o)(o)(o)(o)                |
| Accessibility  | Animation speed    [------o------]  1x              |
| About          | Reduce motion      [ toggle ]                       |
+----------------+-----------------------------------------------------+
```

- Sidebar: Appearance, Desktop, Notifications, Keyboard, Accessibility, About. Rows 32.
  Selected category accent fill + 2 px bar.
- Toolbar: search 220 x 28. Search filters settings rows and jumps to the first match.
  No results: "No settings found" with the query.
- Page title 22/28 600 at the top of content, 28 high, 16 px below.
- 36 px setting row: label 13/18 left, control right (max 320), no border between rows.
  48 px when a description is present: label 13/18 + description 11/14 secondary.
- Toggles use the shared toggle. Segmented controls use the shared 28 px control.
- Appearance: Theme (Dark/Light segmented), Accent (6 swatches 24 px circles, 8 gap,
  selected 2 px ring + check), Animation speed (slider 0..2 with 5 stops), Reduce motion
  (toggle), Night light (toggle + warmth slider, shown only when on).
- Desktop: Dock magnification (toggle). No wallpaper picker beyond theme gradients.
- Notifications: Do not disturb (toggle), Show toasts (toggle). Auto-dismiss stays 5 s;
  no duration setting in v1.
- Keyboard: key repeat delay 250/500/750 ms segmented (default 500), repeat rate
  Slow/Normal/Fast segmented (50/33/20 ms, default Normal). Shortcut reference opens the
  read-only list from INTERACTION.md 1.1.
- Accessibility: UI scale 100/125/150/200 segmented, with unavailable options disabled
  and an inline reason; caret blink toggle (on/off); reduce motion mirrors Appearance.
- About: "Aster OS", version string from the build, "Inter font, SIL Open Font
  License", memory/storage figures only if the kernel exposes them.
- No accounts, network, privacy, telemetry, Store, Update, Installer or Browser pages.
- Changes apply immediately. No Apply/Restart buttons. A setting that requires a restart
  must say so inline; none do in v1.

---

## 11. Cross-app rules

1. **One accent.** Apps use the user's accent. No app-specific chrome colors.
2. **No splash screens, no onboarding carousels, no modal on launch.**
3. **First frame within 200 ms.** If data takes longer, draw the shell and an inline
   loading row; never block the window.
4. **No fabricated data.** Empty, zero and unavailable are distinct states.
5. **No feature theater.** Do not show disabled buttons for unimplemented features.
   Remove them entirely.
6. **No hidden gestures.** Everything is a visible control or a documented shortcut.
7. **No drag-and-drop in v1** between or within apps. Do not show drop targets.
8. **No tabs** outside Terminal.
9. **Notifications** are only for real events or errors from the app. No promotional,
   synthetic or "tip" notifications.
10. **Unsaved changes** use the in-window modal from INTERACTION.md 6.4.
11. **Dialogs** are centered over the owning window, not the screen, and scrim only that
    window.
12. **Keyboard parity** for every action (INTERACTION.md 8.1).
13. **Text** follows the type roles. Editors use the 15/24 reading style.
14. **Scrollbars** are the shared 8 px overlay style, auto-hiding after 1200 ms of no
    scroll, reappearing on hover or scroll.
15. **Window size** is restored per app and per window where the app is multi-instance.

---

## 12. Not implemented - must not be implied

The following are **not implemented in v1** and must not appear as launcher tiles, dock
items, menu items, toolbar buttons, settings pages, notifications, empty states, help
topics, keyboard shortcuts, or "coming soon" labels:

- **Store** - no app store, catalog, install, purchase, or download UI.
- **Installer** - no package installer, file association for packages, or install
  progress UI.
- **Update** - no update check, update badge, changelog, restart-to-update prompt, or
  version-check network call.
- **Browser** - no web browser, URL bar, tabs, bookmarks, history, or "open link" entry
  point.

Also not implemented and not to be implied:

- network, Wi-Fi, Bluetooth, accounts, cloud sync, telemetry
- audio volume, panel brightness, battery, cameras, microphones
- lock screen, screenshots, screen recording, multiple monitors, touch, gestures
- file drag-and-drop, desktop icons, window tabs outside Terminal
- app marketplace badges, "new" dots, or promotional banners

If a future build adds any of these, add a dedicated spec first, then a launcher tile,
then the app. Never add a placeholder first.
