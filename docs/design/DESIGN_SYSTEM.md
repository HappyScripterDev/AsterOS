# Aster OS Design System

Status: normative v1. All values in this document are logical pixels at UI scale 1.0.

Aster OS is drawn entirely by the ring-3 compositor. There are no image assets and no
platform widget toolkit. Every visual in this document must be expressible with:

- filled rounded rectangles, strokes, circles, triangles
- cubic or quadratic bezier paths (icons, star mark)
- source-over alpha blending
- live sampling of the composited backbuffer with a two-pass box blur (glass)
- cached rounded-rectangle shadow masks (no per-frame gaussian blur)
- an Inter bitmap alpha atlas rasterized at 11, 13, 15, 22 and 28 px, weights 400/500/600

Rules in this file override ad-hoc values in app code. If an implementation needs a
value that is not here, add it here first.

---


> **Shipped implementation note (2026-11-20).** Aster Desktop is **light-only**
> by product decision. The dark palette once explored in this document is
> archived and not exposed in the UI. The shipped tokens are:
>
> - canvas `#F2F2F7`, surface `#FFFFFF`, elevated/sidebar `#F5F5F7`/`#F0F0F3`,
>   hairline `#D9D9DE`, text `#1D1D1F`, secondary `#6E6E73`,
>   tertiary `#9A9AA1`;
> - glass `rgba(255,255,255,196)` and the material tokens in
>   `user/asterdesk/src/material.rs` (ULTRA_THIN, THIN, REGULAR, THICK, CHROME,
>   PANEL, SIDEBAR, MENU);
> - live backdrop blur samples the composited backbuffer and is controlled by
>   Settings > Graphics & Motion (transparency and blur toggles are honoured);
> - top-bar workspace pager (click a dot to jump), fullscreen via Meta+F / F11 /
>   Escape, and Alt-Tab cards with live window thumbnails;
> - accents: Blue `#007AFF` (default), Purple `#AF52DE`, Pink `#FF2D55`,
>   Red `#FF3B30`, Orange `#FF9500`, Yellow `#FFCC00`, Green `#34C759`,
>   Graphite `#8E8E93`;
> - traffic lights `#FF5F57`, `#FEBC2E`, `#28C840`;
> - menu bar 26 px, dock 68 px, Spotlight panel 660 px wide, window radius 12 px.
>
> The desktop follows macOS interaction patterns (menu bar, Spotlight, dock,
> traffic-light controls) but all artwork is original or supplied by the
> project. No Apple assets are used.


## 1. Brand

**Aster** means star. The identity is a four-point star with concave sides: precise
geometry, soft light, nothing ornamental.

**Personality:** calm precision, soft light, geometric.

**Design principles**

1. **One accent at a time.** A window or panel uses one accent color for state. Color
   is a signal, not decoration. (Hick's law: fewer simultaneous signals, faster choice.)
2. **Soft light, hard alignment.** Shadows are diffuse and low-contrast; edges, text
   baselines, icons and rows sit on an 4 px grid. Blur is reserved for the wallpaper
   and glass.
3. **Quiet chrome.** Titlebars, toolbars and the dock recede. Content carries contrast.
4. **Direct manipulation.** Large targets at screen edges and corners exploit
   Fitts's law: the dock, top bar and window edges are effectively infinite-depth
   targets.
5. **No imitation.** Do not use Apple, Microsoft or third-party icon shapes,
   wallpapers, sounds, fonts beyond Inter, or copied window layouts. The star mark,
   the monochrome window controls, the top-bar pager and the flat dock are Aster
   specific.

**Voice and copy**

- Sentence case for labels, titles, buttons and menu items.
- No exclamation marks. No marketing adjectives. No "coming soon".
- Verbs for actions: "Open", "Save", "Delete", "Turn off".
- Errors state what happened and what the user can do next, in that order.
- Never claim a feature, device, service or network that does not exist.

**The mark**

24 x 24 construction, scalable. Four tips at (12,2), (22,12), (12,22), (2,12).
Four quadratic segments, each from one tip to the next with its control point at the
center (12,12). Filled, no stroke. At 12 px use the same path; at bar size the mark is
12 x 12. Mark color: `color.text` on chrome, `color.accent` in the launcher header.

---

## 2. Color

### 2.1 Required tokens

These values are fixed. Names are the code-facing token names.

| Token | Dark | Light | Role |
|---|---|---|---|
| `color.canvas` | `#0B0D12` | `#EDF0F5` | Desktop backdrop, app sidebar, terminal background |
| `color.surface` | `#141821` | `#FFFFFF` | Window body, menus, default control surface |
| `color.surface-elevated` | `#1B202A` | `#F7F9FC` | Cards, popovers, raised rows, calculator keys |
| `color.hairline` | `#2A3140` | `#D8DEE8` | Decorative 1 px dividers only |
| `color.text` | `#F2F4F8` | `#12151B` | Primary text and icons |
| `color.text-secondary` | `#A8B0BF` | `#5A6473` | Metadata, labels, placeholder, inactive title |
| `color.text-tertiary` | `#6F7889` | `#8A93A3` | Disabled text, non-essential ornament only |
| `color.glass-tint` | `rgba(18,22,31,0.72)` | `rgba(255,255,255,0.68)` | Glass panels over wallpaper |

Accents:

| Token | Value | Typical use |
|---|---|---|
| `color.accent` | `#5B8CFF` | Default accent, focus, selection, primary action |
| `color.accent-violet` | `#A78BFA` | Alternate accent, charts, tags |
| `color.accent-teal` | `#2DD4BF` | Alternate accent, success-adjacent data |
| `color.accent-rose` | `#FB7185` | Alternate accent, media and creative apps |
| `color.accent-amber` | `#FBBF24` | Alternate accent, warnings, time |
| `color.accent-green` | `#34D399` | Alternate accent, health and completion |

Semantic:

| Token | Value |
|---|---|
| `color.success` | `#34D399` |
| `color.warning` | `#FBBF24` |
| `color.danger` | `#F87171` |
| `color.info` | `#60A5FA` |

Only one accent may be active in a window at a time. The system default is
`#5B8CFF`. Apps do not recolor their chrome per app; accent choice is a user setting.

### 2.2 Contrast audit (WCAG 2.2 AA)

Measured sRGB contrast ratios. AA requires 4.5:1 for normal text, 3:1 for large text
(>= 24 px regular or >= 18.66 px bold) and 3:1 for non-text UI boundaries and states.

Text on surfaces:

| Foreground | On `canvas` dark | On `surface` dark | On `surface-elevated` dark | Verdict |
|---|---|---|---|---|
| `text` #F2F4F8 | 17.65 | 16.13 | 14.82 | AA/AAA |
| `text-secondary` #A8B0BF | 8.91 | 8.14 | 7.48 | AA/AAA |
| `text-tertiary` #6F7889 | 4.37 | 4.00 | 3.67 | Fails AA normal; large text only |

| Foreground | On `canvas` light | On `surface` light | On `surface-elevated` light | Verdict |
|---|---|---|---|---|
| `text` #12151B | 16.00 | 18.28 | 17.33 | AA/AAA |
| `text-secondary` #5A6473 | 5.24 | 5.99 | 5.68 | AA/AAA |
| `text-tertiary` #8A93A3 | 2.71 | 3.10 | 2.94 | Fails AA normal and large; non-text only on white |

**Consequence:** `text-tertiary` never carries primary or metadata text. Use
`text-secondary` for timestamps, placeholders, helper text, inactive titles and menu
shortcuts. `text-tertiary` is allowed only for disabled content (exempt) and decorative
glyphs that duplicate a visible label. Large `display`/`headline` text may use tertiary
on dark surfaces (3.67:1), never on light.

Accent text:

| Accent | On dark `surface-elevated` | On light `surface-elevated` |
|---|---|---|
| blue #5B8CFF | 5.16 AA | 3.00 fail |
| violet #A78BFA | 6.00 AA | 2.58 fail |
| teal #2DD4BF | 8.77 AA | 1.76 fail |
| rose #FB7185 | 6.06 AA | 2.55 fail |
| amber #FBBF24 | 9.78 AA | 1.58 fail |
| green #34D399 | 8.49 AA | 1.82 fail |

On light surfaces use the derived strong variants in 2.3 for accent-colored text.
Semantic text on dark `surface-elevated`: success 8.49, warning 9.78, danger 5.90,
info 6.42. All pass AA. Semantic text on light must use the strong variants.

Filled controls: dark ink `#0B0D12` on the bright accents measures 6.15 (blue), 7.14
(violet), 10.44 (teal), 7.22 (rose), 11.64 (amber), 10.11 (green). All pass AA.
White on the bright accents is only 1.67 to 3.16 and must never label a bright fill.

### 2.3 Derived accessibility tokens

These are additions required to make the fixed palette pass AA. They do not replace any
required token.

| Token | Dark | Light | Use |
|---|---|---|---|
| `color.border-control` | `#626E82` | `#7E8B9D` | 1 px outline of inputs, checkboxes, radios, sliders, toggles |
| `color.focus-ring` | `#5B8CFF` | `#2A57C4` | 2 px focus indicator |
| `color.accent-strong` | n/a | `#2A57C4` | Blue text and filled controls on light |
| `color.accent-violet-strong` | n/a | `#6D4AD6` | Violet text/fill on light |
| `color.accent-teal-strong` | n/a | `#0F766E` | Teal text/fill on light |
| `color.accent-rose-strong` | n/a | `#BE3455` | Rose text/fill on light |
| `color.accent-amber-strong` | n/a | `#8A5A00` | Amber text/fill on light |
| `color.accent-green-strong` | n/a | `#067A55` | Green text/fill on light |
| `color.success-text` | `#34D399` | `#067A55` | Success copy |
| `color.warning-text` | `#FBBF24` | `#8A5A00` | Warning copy |
| `color.danger-text` | `#F87171` | `#B42318` | Error copy, destructive labels |
| `color.info-text` | `#60A5FA` | `#1D5FBF` | Informational copy |

Measured:

- `border-control` dark: 3.44 on `surface`, 3.16 on `surface-elevated`, 3.77 on `canvas`.
- `border-control` light: 3.46 on `surface`, 3.28 on `surface-elevated`, 3.03 on `canvas`.
- `focus-ring` dark: 5.62 on `surface`, 5.16 on elevated. Light `#2A57C4`: 6.44 on
  white, 6.11 on elevated, 5.64 on canvas.
- Light strong accents as text: blue 6.11, violet 5.53, teal 5.19, rose 5.22, amber
  5.62, green 5.08 (all on `surface-elevated`).
- White on strong accent fills: blue 6.44, violet 5.83, teal 5.47, rose 5.51, amber
  5.93, green 5.35. Use white labels on these.
- Light semantic strong fills with white: success 5.35, warning 5.93, danger 6.57,
  info 6.10.

Filled control rule:

- Dark theme: fill = bright accent or semantic color, label/icon = `#0B0D12`.
- Light theme: fill = strong accent or strong semantic, label/icon = `#FFFFFF`.
- Focus ring is always drawn in the 2 px gap over the surrounding surface, so the
  ring's contrast is measured against that surface, not the fill.

### 2.4 Borders, overlays, selection

| Token | Dark | Light | Notes |
|---|---|---|---|
| `color.hairline` | `#2A3140` | `#D8DEE8` | 1.25-1.36:1. Dividers only, never the only affordance of a control |
| `color.window-border` | `rgba(255,255,255,0.10)` | `rgba(18,22,31,0.10)` | Focused window outer edge |
| `color.window-border-inactive` | `rgba(255,255,255,0.06)` | `rgba(18,22,31,0.08)` | Unfocused window |
| `color.panel-border` | `rgba(255,255,255,0.10)` | `rgba(18,22,31,0.10)` | Opaque panels |
| `state.hover-overlay` | `rgba(255,255,255,0.06)` | `rgba(0,0,0,0.05)` | Layered over the component fill |
| `state.pressed-overlay` | `rgba(255,255,255,0.10)` | `rgba(0,0,0,0.09)` | Layered over the component fill |
| `state.selected-fill` | `rgba(91,140,255,0.16)` | `rgba(91,140,255,0.14)` | List rows, active dock-less tiles |
| `state.disabled-alpha` | `0.38` | `0.38` | Multiply whole component incl. text |
| `color.scrim` | `rgba(5,7,11,0.48)` | `rgba(18,22,31,0.28)` | In-window modal scrim |
| `color.launcher-scrim` | `rgba(5,7,11,0.35)` | `rgba(18,22,31,0.18)` | Over the blurred wallpaper field |

Primary filled buttons darken by `rgba(0,0,0,0.08)` on hover and
`rgba(0,0,0,0.14)` on press in both themes.

### 2.5 Color behavior rules

1. Never encode state with color alone. Pair it with a shape change, an icon, a label,
   a 2 px bar or a weight change.
2. Text over glass must be measured against the lightest wallpaper region it can cover.
   The required tints guarantee >= 14:1 for `text` on the wallpaper gradients in
   section 6.4; do not lower the tint alpha.
3. Disabled components keep their geometry. Do not change border widths on disable.
4. Do not animate hue. Theme changes crossfade the whole palette over 200 ms.
5. Charts use accent at 100% for the primary series, accent at 40% for the secondary,
   and semantic colors only for thresholds. Grid lines use `hairline`.

---

## 3. Typography

### 3.1 Font and atlas

- Family: Inter (SIL Open Font License). No other text face ships.
- Rasterized alpha atlas at 11, 13, 15, 22 and 28 px, weights 400, 500, 600.
- Coverage is 8-bit; glyphs are blended as coverage, never color-keyed.
- Digits exist as proportional and tabular variants at 13 px and 22 px. Use tabular
  digits for clocks, timers, calculator, tables and aligned numeric columns.
- Missing codepoints render as a hollow 1 px box glyph. Never render silently blank.
- Upright only: no italic, condensed or synthetic bold in v1. 600 is a real atlas weight.
- Text is left aligned. Right alignment is allowed only for numeric columns, the clock
  and calculator display. Do not justify.

### 3.2 Roles

| Role | Size / line | Weight | Tracking | Use |
|---|---|---|---|---|
| `type.caption` | 11 / 14 | 500 | +0.01em (~0.1 px) | Dense labels above values, badge counts, table units |
| `type.small` | 12 / 16 | 500 | 0 | Menu shortcuts, timestamps, tooltips, secondary metadata |
| `type.body` | 13 / 18 | 400 | 0 | Default body, list rows, inputs, terminal |
| `type.body-medium` | 13 / 18 | 600 | 0 | Buttons, row titles, selected labels, window title |
| `type.title` | 15 / 20 | 600 | -0.005em (~-0.1 px) | Panel headers, dialog titles, section leads |
| `type.headline` | 22 / 28 | 600 | -0.01em (~-0.2 px) | Settings page title, empty-state headline, month label |
| `type.display` | 28 / 34 | 600 | -0.015em (~-0.4 px) | Calculator display, large numeric readout |

**Known constraint:** the atlas has no 12 px face. `type.small` is rendered from the
13 px atlas bitmap scaled to 12 px (factor 0.923) with bilinear sampling and integer
baseline snapping. If this proves too soft, replace `type.small` with 13/18 rather than
adding a sixth atlas size. Do not silently use 13 px with a 16 px line height: the
12/16 metrics are part of the layout.

**App-specific reading role:** long-form editors (Text, Notes) use 15 px with a 24 px
line box, weight 400. This is the only sanctioned line height outside the table above.
It exists because 13/18 is too dense for reading and 15/20 is too tight.

### 3.3 Paragraph and truncation rules

- Paragraph spacing: 8 px. Do not use first-line indents.
- Body measure: 60-75 characters, hard maximum 680 px in Text and Notes.
- Single-line labels truncate with a trailing ellipsis drawn as three 1 px dots with
  2 px gaps; reserve 12 px for it.
- Notifications and list rows clamp to 2 lines maximum, then ellipsis.
- Never clip a glyph vertically. Line boxes are exact multiples of the values above.
- Window titles: one line, ellipsis, max width `window.width - 200`.

---

## 4. Space, size and radius

### 4.1 Spacing scale

Approved values: 4, 8, 12, 16, 20, 24, 32, 40, 48, 64.

| Token | Value | Use |
|---|---|---|
| `space.1` | 4 | Icon-to-label in dense rows, menu padding |
| `space.2` | 8 | Control inner padding, related items, gaps in groups |
| `space.3` | 12 | Toolbar gaps, card padding, shell padding |
| `space.4` | 16 | Section padding, panel padding |
| `space.5` | 20 | App content padding, dialog padding |
| `space.6` | 24 | Section separation, window cascade step |
| `space.8` | 32 | Large blocks, empty-state gaps |
| `space.10` | 40 | Page top padding |
| `space.12` | 48 | Window edge inset at large sizes |
| `space.16` | 64 | Marketing-free hero spacing; rarely used in apps |

Alignment: all x/y positions of rectangular elements snap to a multiple of 4 logical px
unless the value is a computed center or a fractional grid track. Text baselines are
not snapped to the 4 px grid; line boxes are.

### 4.2 Radii

| Token | Value | Use |
|---|---|---|
| `radius.6` | 6 | Menu items, tooltips, compact fields, small buttons |
| `radius.10` | 10 | Menus, standard text fields, popovers |
| `radius.14` | 14 | Windows, cards, launcher tiles, notification cards |
| `radius.18` | 18 | Dock, control center, notification center, switcher |
| `radius.24` | 24 | Launcher panel, large overlay sheets |
| `radius.pill` | 999 | Pills, badges, toggles, scrollbar thumbs, circles |

Nested radius: `inner = max(6, outer - inset)`. Never nest a larger radius inside a
smaller one.

### 4.3 Fixed heights and hit sizes

| Element | Value |
|---|---|
| Top bar | 28 |
| Window titlebar | 28 |
| App toolbar | 44 |
| List row | 36 |
| Sidebar navigation row | 32 |
| Standard button | 28 or 32 |
| Text field | 32 (compact 28) |
| Menu row | 28 |
| Toggle track | 36 x 20 (hit 44 x 28) |
| Checkbox / radio | 16 visual (hit 28 x 28) |
| Slider track | 4 (hit 28) |
| Scrollbar | 8 track, 4 thumb |
| Dock (resting) | 64 tall, 40 px icons |
| Minimum pointer target | 28 x 28 |

Minimum target spacing: 8 px between adjacent targets where the layout allows it. Menu
rows, list rows and segmented controls are exempt because each row is a full-width
target 28 px or taller.

---

## 5. Elevation and shadows

Light comes from above, slightly forward. Shadows are broad, low-alpha and neutral. No
shadow is pure black in light theme.

Because the compositor renders shadows from a cached mask, every window shadow is
generated once per size (`gfx::shadow_mask`), then composited with integer
source-over alpha. The mask is a rounded-rectangle distance field blurred at the
quality mode's radius; it is not regenerated per frame.

Shadow color: dark `rgba(0,0,0,a)`; light `rgba(23,32,48,a)`.

### 5.1 Elevation tokens

| Token | Use | Canonical (y / blur / alpha) |
|---|---|---|
| `elev.1` | Tooltips | 2 / 6 / 0.28 dark, 0.16 light |
| `elev.2` | Menus, popovers, toasts | 6 / 16 / 0.31 dark, 0.19 light |
| `elev.3` | Panels, dock, launcher, switcher | 12 / 28 / 0.37 dark, 0.21 light |
| `elev.4` | Focused window | 18 / 40 / 0.45 dark, 0.26 light |
| `elev.4-inactive` | Unfocused window | 8 / 24 / 0.29 dark, 0.17 light |

The canonical numbers describe the blur intent; the shipped light renderer
realizes them with the cached mask. The band tables below are retained as the
historical dark-theme fallback specification.

**elev.1 - dark**

| Band | dy | Inflation | Alpha |
|---|---|---|---|
| 1 | 1 | 1 | 0.12 |
| 2 | 2 | 3 | 0.10 |
| 3 | 3 | 6 | 0.08 |

Cumulative core 0.271. Light: 0.07 / 0.06 / 0.04, cumulative 0.160.

**elev.2 - dark**

| Band | dy | Inflation | Alpha |
|---|---|---|---|
| 1 | 2 | 2 | 0.16 |
| 2 | 4 | 6 | 0.11 |
| 3 | 8 | 16 | 0.08 |

Cumulative core 0.313. Light: 0.09 / 0.06 / 0.05, cumulative 0.187.

**elev.3 - dark**

| Band | dy | Inflation | Alpha |
|---|---|---|---|
| 1 | 3 | 4 | 0.20 |
| 2 | 7 | 12 | 0.14 |
| 3 | 14 | 28 | 0.09 |

Cumulative core 0.374. Light: 0.10 / 0.08 / 0.05, cumulative 0.214.

**elev.4 - focused window, dark**

| Band | dy | Inflation | Alpha |
|---|---|---|---|
| 1 | 4 | 6 | 0.25 |
| 2 | 10 | 18 | 0.17 |
| 3 | 20 | 40 | 0.11 |

Cumulative core 0.446. Light: 0.13 / 0.10 / 0.06, cumulative 0.264.

**elev.4-inactive - dark**

| Band | dy | Inflation | Alpha |
|---|---|---|---|
| 1 | 3 | 4 | 0.15 |
| 2 | 7 | 10 | 0.10 |
| 3 | 14 | 24 | 0.07 |

Cumulative core 0.288. Light: 0.08 / 0.06 / 0.04, cumulative 0.169.

### 5.2 Window dressing

- Focused window outer border: 1 px `color.window-border`.
- Unfocused: 1 px `color.window-border-inactive`.
- Dark theme only: 1 px inner top highlight `rgba(255,255,255,0.06)`, inset 1 px,
  spanning the top edge. Light theme has no inner highlight.
- Maximized windows: no shadow, radius 0, 1 px border only.
- Shadows are never hit-testable and never clipped by the work area; they may draw
  into the dock band but the dock is above them.
- Elevation does not change on hover. It changes only with focus and maximize state.
- Do not stack a card shadow (`elev.2`) inside a window and call it elevation; cards in
  windows use a 1 px border and a 1 step tonal shift instead.

### 5.3 Card recipe

`surface-elevated`, radius 14, 1 px `rgba(255,255,255,0.06)` dark or
`rgba(18,22,31,0.08)` light, no shadow. Hover adds `state.hover-overlay`; selected adds
the selected fill and a 2 px accent bar on the leading edge (inside the radius).

---

## 6. Glass

### 6.1 Where glass is allowed

Live glass samples the **composited backbuffer**, so it works over the wallpaper,
over windows and over the dimmed desktop. It is used for:

- Top bar, workspace pager and dock.
- Launcher panel, Control Centre, notification panel and toasts.
- Menus and popovers.
- Settings cards that float above the desktop.

Glass is never nested inside another glass surface. Panels that must stay
legible over arbitrary window content use `THICK`, `PANEL` or `MENU`, whose tint
alpha is at least 226/255. Thin materials are reserved for the top bar and dock,
which normally sit over the wallpaper.

### 6.2 Material tokens

Defined in `user/asterdesk/src/material.rs`:

| Token | Tint (white alpha) | Blur radius | Downsample | Use |
|---|---|---|---|---|
| `ULTRA_THIN` | 138 | 3 | 4 | decorative chips only |
| `THIN` | 178 | 3 | 3 | secondary chrome |
| `REGULAR` | 205 | 3 | 3 | dock, window-adjacent cards |
| `THICK` | 232 | 4 | 2 | launcher, modal panels |
| `CHROME` | 218 | 2 | 4 | top bar |
| `PANEL` | 226 | 4 | 2 | Control Centre, notifications |
| `SIDEBAR` | 235 | 2 | 4 | sidebars |
| `MENU` | 244 | 4 | 2 | menus |

Each token also carries a saturation multiplier, a deterministic +/-1 LSB noise
amplitude, a 1 px inner highlight and a 1 px border. The table lists the shipped
light-theme tints; the dark column is archived.

### 6.3 Blur pipeline

Computed per painted panel, never for the whole screen:

1. Capture the panel rectangle from the composited backbuffer. This already
   contains the wallpaper and every window behind the panel.
2. Downsample by the token factor (2-4x) into a reusable scratch buffer.
3. Run two box-blur passes, horizontal then vertical, shrinking the radius by
   25% on the second pass.
4. Re-sample while drawing, apply saturation, add the +/-1 LSB dither and mix
   the token tint.
5. Draw the rounded 1 px inner highlight and border.

The scratch buffers (`glass_small`, `glass_scratch`) belong to `Desktop`, grow
once and are reused for every panel, so no allocation happens inside the frame
loop. Quality modes adjust downsample and radius: Performance reduces both,
Balanced uses the token values and Quality raises the radius by one.

`reduce_transparency` bypasses sampling entirely and fills the panel with the
opaque surface color plus the token border. `reduce_motion` changes animation
timing only, never the blur.

Text over glass must be measured against the lightest wallpaper region behind
the panel. `color.text` on a `THICK`/`PANEL`/`MENU` composite stays above 7:1;
never place `text-tertiary` on `ULTRA_THIN` or `THIN`.

### 6.4 Wallpaper recipe

Generated procedurally, no image file. Ordered 4x4 Bayer dither at 1 LSB to prevent
banding.

**Dark**

- Vertical linear gradient: `#0E1420` at y=0 to `#0B0D12` at y=H.
- Radial glow A: center (0.50 W, 0.36 H), radii (0.48 W, 0.52 H),
  `rgba(91,140,255,0.10)` center to 0 at edge, smoothstep falloff.
- Radial glow B: center (0.18 W, 0.78 H), radii (0.30 W, 0.34 H),
  `rgba(167,139,250,0.06)` center to 0.
- Dither.

**Light**

- Vertical linear gradient: `#F4F7FC` at y=0 to `#E9EEF6` at y=H.
- Radial glow A: center (0.50 W, 0.28 H), radii (0.50 W, 0.46 H),
  `rgba(91,140,255,0.10)` center to 0.
- Radial glow B: center (0.82 W, 0.72 H), radii (0.34 W, 0.38 H),
  `rgba(167,139,250,0.06)` center to 0.
- Dither.

The wallpaper is decorative; no alt text concept is needed, but the desktop must remain
usable if it fails to draw: fall back to a flat `color.canvas`.

---

## 7. Icons

### 7.1 Grid

- Box: 24 x 24. Live area: 20 x 20 centered, 2 px padding all around.
- Stroke: 2 px, centered on the path, round caps and round joins.
- Corner radius for square shapes: 6 px (25% of the box).
- Fill: none, except 2-4 px dots used as indicators. No gradients. No two-tone icons.
- Keylines:
  - square: 18 x 18, radius 6
  - circle: 20 diameter
  - portrait: 16 x 20, radius 6
  - landscape: 20 x 16, radius 6
  - diagonal: tip-to-tip within the 20 x 20 live area
- Snap stroke edges to physical pixels where possible. Anti-aliasing is allowed and
  expected.

### 7.2 Optical sizes

Stroke width is normalized optically, not scaled blindly:

| Render size | Stroke | Use |
|---|---|---|
| 12 | 1.5 | Star mark only |
| 14 | 1.5 | Status glyphs, chevrons, table sort |
| 16 | 1.5 | Toolbar, menu, inline icons |
| 20 | 2 | Launcher tiles, control center, window control glyphs |
| 24 | 2 | Full-size icons, empty states |
| 40 | 2.5 | Dock icons |

At 40 px the 24 px path is scaled by 1.667 and the stroke is reduced optically to 2.5 px
so icons do not look heavier than toolbar icons. Hit boxes are independent of icon size.

### 7.3 System glyph set

All are 24-grid paths drawn procedurally.

| Glyph | Construction |
|---|---|
| star mark | 4 concave quadratic tips (section 1) |
| search | circle r=6 at (10,10), 2 px handle to (20,20) |
| close | X from (6,6) to (18,18), 2 px, round |
| minimize | horizontal 2 px line (6,12) to (18,12) |
| maximize | 12 x 12 square, radius 3, centered |
| restore | two offset 10 x 10 squares, radius 3 |
| chevron | 8 x 8 L rotated 45 degrees, 2 px |
| plus | 12 px cross, 2 px |
| check | 4 px in, 12 px long diagonal, 2 px |
| minus | 12 px line |
| bell | 14 x 12 rounded bell silhouette, 2 px; 4 px clapper dot |
| sun | circle r=4, 8 rays 2 px |
| moon | 14 px circle with a 12 px offset cut (two-path even-odd fill) |
| gear | 8-tooth cog, outer r=10, inner r=4.5, 2 px |
| trash | lid line, 12 x 14 can, 2 px |
| folder | two rounded rects, 18 x 14 |
| terminal | 18 x 16 rounded rect + `>` + 4 px underscore |
| file | page 14 x 18, folded corner triangle 5 px |
| calendar | 18 x 16 rounded rect, 2 binding dots, 3 x 3 grid |
| image | 18 x 14 rounded rect + r=2 sun + triangle mountain |
| chart | 2 axes + 3 vertical bars |
| arrow | 12 px shaft + 6 px head, 2 px, four rotations |
| refresh | 14 px arc with a 4 px arrowhead |
| copy | two offset 10 x 12 rounded rects, radius 2 |
| sliders | 3 horizontal 2 px lines with 3 px knobs at staggered x |
| pause | two 3 x 12 bars, 3 px apart |
| expand | four 5 px corner brackets |
| more | three 3 px dots, 3 px apart |

Icon-only buttons require a tooltip. An icon is never the only label for a destructive
action.

### 7.4 App icon metaphors

Dock and launcher icons use the same 24 grid, 2 px stroke, monochrome `color.text`, with
no app-specific accent. They must remain distinguishable at 20 px (control center) and
40 px (dock).

| App | Construction |
|---|---|
| Files | folder: 18 x 14 rounded rect with a 8 x 4 tab at the top left |
| Terminal | 18 x 16 rounded rect, 5 px `>` chevron at left, 4 px underscore at right |
| Text | page 14 x 18 with a 10 px horizontal bar at the top and a 10 px vertical stem below it (a drawn "T") |
| Notes | page 14 x 18 with a 2 px top binding and three 8 px lines; a 3 px accent is not used |
| Calendar | 18 x 16 rounded rect, two 2 px binding marks at the top, 3 x 3 dot grid |
| Photos | 18 x 14 rounded rect, 6 px sun circle, 2 px mountain triangle |
| Calculator | 16 x 18 rounded rect, 10 x 3 display bar, 3 x 3 dot grid below |
| Monitor | 18 x 14 rounded rect with a 10 px polyline (up, down, flat) |
| Settings | 8-tooth gear, outer r=9, inner r=4, 2 px |

---

## 8. Motion

### 8.1 Tokens

| Token | Duration | Easing |
|---|---|---|
| `motion.fast` | 120 ms | standard `cubic-bezier(0.2, 0, 0, 1)` |
| `motion.base` | 200 ms | standard |
| `motion.slow` | 320 ms | standard |
| `motion.enter` | enter curve `cubic-bezier(0.16, 1, 0.3, 1)` at the same durations | enter |
| `motion.exit` | exit curve `cubic-bezier(0.4, 0, 1, 1)` at the same durations | exit |

No bounce, no elastic, no overshoot. Motion confirms causality; it does not entertain.

### 8.2 Choreography

| Event | Change | Duration / curve |
|---|---|---|
| Window open | scale 96% -> 100% (0.96 -> 1.00), opacity 0 -> 1 | 200 enter |
| Window close | scale 100% -> 96%, opacity 1 -> 0 | 120 exit |
| Window minimize | interpolate rect to dock icon rect, radius 14 -> 10, fade | 200 standard |
| Window restore | reverse minimize | 200 enter |
| Maximize / restore | rect interpolation, radius 14 <-> 0 | 200 standard |
| Snap | rect interpolation to target | 200 standard |
| Dock magnification | per-frame target, exponential smoothing tau 60 ms | continuous |
| Launcher | backdrop fade; panel scale 0.98 -> 1 + fade | 200 scrim, 120 enter |
| Control center / notification center | translate y -4 -> 0, fade | 120 enter, 120 exit |
| Menu | translate y -4 -> 0, fade | 120 enter, 120 exit |
| Tooltip | fade only | 120 enter, 0 exit |
| Toast in / out | translate y -8 -> 0 + fade / reverse | 200 enter, 120 exit |
| Desktop switch | slide 48 px + crossfade | 320 standard |
| Hover | color/overlay only | 120 standard |
| Press | scale 1 -> 0.98 for buttons and tiles | 120 standard |
| Focus ring | fade 0 -> 1 | 120 standard |
| Theme switch | palette crossfade, wallpaper crossfade | 200 / 320 |
| Notification auto-dismiss | not an animation; 5000 ms real time | n/a |

### 8.3 Rules

- Animations are time-based, never frame-count based. Compute progress from
  `(now - start) / duration`; clamp 0..1.
- The compositor is capped at 60 FPS. Clamp `dt` at 32 ms to avoid a jump after a stall.
- Do not animate blur radius, shadow band count, or layout reflow of text.
- Interpolate rectangles, scale, opacity and color in sRGB. Interpolate radii linearly.
- Only one element group should move at a time in a transition; the dock may magnify
  while a window minimizes, but no other decorative motion may compete.
- Animations do not block input. A window can be clicked during its open animation.

### 8.4 Speed multiplier and reduce motion

User setting `animation_speed` ranges 0 to 2. Stops: Off (0), 0.5x, 1x, 1.5x, 2x.

```
effective_duration = token_duration / animation_speed   when speed > 0
effective_duration = 0                                  when speed = 0
```

`reduce_motion` (boolean) overrides transforms:

- All scale, translate, slide and magnification animations become instant.
- Opacity may still fade, capped at 120 ms.
- Dock magnification is disabled: icons stay at 1.0 scale with a 2 px upward lift on
  hover. This is a position change, not a scale animation, so it is allowed.
- Desktop switching is an instant cut.
- Window minimize/restore is an instant cut to the final rect.
- Text caret does not blink; it stays solid.
- If both `reduce_motion` and speed 0 are set, every duration is 0.

The multiplier does not affect the 5 s notification timer, tooltip delay, or key repeat.

---

## 9. Focus

### 9.1 Focus ring

- 2 px solid `color.focus-ring` (dark `#5B8CFF`, light `#2A57C4`).
- 2 px offset, drawn outside the component, over the 2 px gap.
- Ring radius: `component radius + 4` so the outer edge stays parallel.
- Never 1 px. Never a dashed ring. Never rely on a border color change alone.
- For components clipped by a scrolling container, draw the ring inset 2 px inside the
  component and keep it fully visible.
- For 12 px window-control circles, draw the ring at 0 offset (16 px outer diameter) so
  neighbors at 20 px pitch do not collide.
- Focus ring fades in over 120 ms; it is never animated on scroll.

### 9.2 Focus visibility policy

- Show the ring whenever focus moves by keyboard, or when a text field receives focus.
- Pointer press on a button may suppress the ring until the next key press.
- Programmatic focus (opening the launcher, a dialog, or returning from a menu) shows
  the ring only if the previous input modality was keyboard.
- Focus is never hidden behind an overlay. Scroll the focused element into view with an
  8 px margin before showing the ring.

### 9.3 Focus order

- Follow visual order: top bar, window content, dock is not in the tab order.
- Within a window: toolbar, sidebar, content, status area.
- One tab stop per composite widget; arrows move inside (tabs, radio groups, grids,
  slider, toggle groups).
- Esc returns focus to the invoking control. Closing an overlay returns focus to the
  element that opened it.

---

## 10. Accessibility

Target: WCAG 2.2 AA where the platform can express it. Aster OS has no assistive
technology bridge in v1; the UI must not claim screen-reader support, voice control, or
captions.

### 10.1 Contrast

- Body, labels, metadata, placeholders: >= 4.5:1.
- Large text (display, headline) and non-text UI: >= 3:1.
- Focus indicator, input boundaries, toggle state, selection: >= 3:1.
- Disabled content is exempt but must remain readable: 38% alpha is the floor.
- `text-tertiary` is not used for active text (section 2.2).
- Never place white text on a bright accent fill (section 2.2).

### 10.2 Target size

- Minimum 28 x 28 logical px, exceeding WCAG 2.2's 24 x 24 minimum.
- 8 px separation where layout permits.
- Documented exceptions: window controls are 20 x 28 with equivalent keyboard actions
  and menu items; resize edges are 6 px with equivalent snap/maximize actions; inline
  text links are exempt as inline content.
- A 28 px target may be larger than its visual: draw 12-16 px glyphs centered in a
  28 x 28 transparent hit box.

### 10.3 Color independence

- Selection = tinted fill + 2 px leading accent bar (not fill alone).
- Error = danger border + 12 px icon + text (not red alone).
- Toggle = thumb position + track fill (not color alone).
- Running app = 4 px dot (unfocused) or 12 px pill (focused), not color alone.
- Charts label thresholds with text and pattern, not hue alone.

### 10.4 Motion and timing

- Honor `reduce_motion` and `animation_speed` (section 8.4).
- No flashing above 3 Hz.
- Toasts auto-dismiss after 5 s, pause on hover and keyboard focus, and always persist
  in the notification center. Auto-dismiss is not the only way to read the content.
- Tooltips are supplemental; no essential information exists only in a tooltip.

### 10.5 UI scaling

Supported: 100%, 125%, 150%, 200%.

| Scale | Minimum physical resolution | Resulting logical size |
|---|---|---|
| 100% | 1280 x 800 | 1280 x 800 |
| 125% | 1280 x 800 | 1024 x 640 |
| 150% | 1440 x 900 | 960 x 600 |
| 200% | 1920 x 1200 | 960 x 600 |

- Layout metrics multiply by S and round to the nearest physical pixel.
- Hairlines and strokes snap to 1 physical pixel minimum.
- Text bitmaps are scaled by S with bilinear sampling. Do not substitute a larger atlas
  size, which would destroy hierarchy.
- Hit testing uses physical pixels; the 28 px minimum is `28 * S` physical pixels.
- Below the minimum logical size of 960 x 600, refuse the scale with a plain message in
  Settings instead of clipping the desktop.

### 10.6 Keyboard and text

- Every action available by pointer is available by keyboard except direct
  manipulation of window edges (covered by snap and maximize shortcuts).
- Visible focus at all times.
- Text selection color: accent at 30% alpha; selected text keeps its `text` color.
- Caret: 1 px, `color.accent`, 530 ms on / 530 ms off, resets to solid on keypress.
  Reduce motion makes it solid.
- No keyboard trap. Esc always closes the top transient surface.

---

## 11. Token index

Code-facing names used across the platform:

```
color.canvas
color.surface
color.surface-elevated
color.hairline
color.text
color.text-secondary
color.text-tertiary
color.glass-tint
color.accent
color.accent-violet
color.accent-teal
color.accent-rose
color.accent-amber
color.accent-green
color.success
color.warning
color.danger
color.info
color.border-control
color.focus-ring
color.window-border
color.window-border-inactive
color.panel-border
color.accent-strong
color.accent-violet-strong
color.accent-teal-strong
color.accent-rose-strong
color.accent-amber-strong
color.accent-green-strong
color.success-text
color.warning-text
color.danger-text
color.info-text
state.hover-overlay
state.pressed-overlay
state.selected-fill
state.disabled-alpha
color.scrim
color.launcher-scrim

type.caption
type.small
type.body
type.body-medium
type.title
type.headline
type.display
type.reading

space.1 .. space.16
radius.6 radius.10 radius.14 radius.18 radius.24 radius.pill

elev.1 elev.2 elev.3 elev.4 elev.4-inactive

motion.fast motion.base motion.slow
ease.standard ease.enter ease.exit
```

---

## 12. Known gaps

- 12 px type is scaled from the 13 px atlas (section 3.2).
- The atlas has no monospace face; Terminal uses a fixed-cell Inter grid (see
  APP_GUIDELINES.md).
- No high-contrast theme and no screen reader bridge in v1; reduce-transparency
  and reduce-motion are real Settings > Graphics & Motion toggles.
- Glass blur is a software box blur over the composited backbuffer. There is no
  GPU blur or GPU rasterization path; see the 0.5.0 limitations register.
- HiDPI scales above 1.0 and multi-monitor layouts are not part of the base
  release; see `docs/releases/0.5.0/KNOWN_LIMITATIONS.md`.
- The lock screen, Store, Installer, Update and Browser are not implemented and must
  not appear in the UI.
