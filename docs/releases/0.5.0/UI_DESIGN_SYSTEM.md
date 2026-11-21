# Aster OS 0.5.0 - UI Design System

## 1. Tokens and rules

Existing Aster tokens remain the source of truth:

- light surfaces: white `#FFFFFF`, elevated `#F5F5F7`, sidebar `#F0F0F3`;
- menu/glass tint: translucent white;
- radii: 6 px small, 10-12 px panels/windows, 18-20 px dock/launcher;
- typography: `SMALL`, `CAPTION`, `BODY`, `BODY_MEDIUM`, `HEADLINE`, `TITLE`,
  `MONO`;
- spacing: 8/12/16/24 px rhythm; cards use 12 px inner padding;
- window title height 30 px; menu bar 26 px; dock icon 50 px at 100% size.

Accent colors are Blue, Purple, Pink, Red, Orange, Yellow, Green and Graphite.
The active accent drives buttons, selection, focus outlines and the workspace
indicator.

## 2. Motion rules

- Durations: 120-280 ms for UI transitions; no long decorative loops.
- Easing: CubicOut for entrances, CubicIn for exits, CubicInOut for geometry,
  Smoothstep/linear for indicators.
- Entrances fade/slide in; exits fade/slide out; nothing bounces.
- Reduced Motion removes geometry/opacity animation and keeps state changes
  immediate.
- Input never waits for an animation.

## 3. Components

| Component | State |
|---|---|
| Button / icon button | Hover/press color and primary/secondary styles; no scale transition |
| Toggle | On/off track and knob; animated only by the caller, currently immediate |
| Slider | Track, fill, thumb; immediate drag feedback |
| Card/panel | White/elevated surfaces, 1 px hairline, rounded corners |
| Menu | Fade-in and fade-out, hover row, shortcuts, checked state |
| Dialog/modal | Drawn surfaces; backdrop transition not implemented |
| Tabs | Tab bar, active underline, close icon, "+" button, dirty marker |
| Scrollbar | Drawn by app; Text uses smooth Page Up/Down |
| Toast | Slide/fade in, fade/slide out, stack of three |
| Tooltip | Immediate menu-bar/dock tooltip |
| Focus | Accent outline on selected overview cards/switcher items |

## 4. Tabs

The Text editor introduces the reusable tab pattern:

- active tab uses the surface color with an accent underline;
- inactive tabs use a muted elevated color;
- dirty files append `*`;
- close icon per tab, `+` creates a tab;
- clicking a tab selects it, clicking its close icon closes it;
- keyboard: Ctrl+T new, Ctrl+PageUp/PageDown cycle, Ctrl+1..8 select,
  Ctrl+F4 close.

Tab overflow shrinks tab widths down to 72 px and limits the visible set to
eight; there is no scroll/overflow menu and no drag reorder yet.

## 5. Graphics & Motion page

The Settings section is a real control surface, not a preview mock:

- four performance modes;
- animation speed;
- dock size;
- dock magnification;
- frame limit 30/60/120/Free;
- dock position Bottom/Left/Right;
- ten switches: window, dock, workspace and UI animations, smooth scrolling,
  shadows, transparency, blur, dock indicators, dock auto-hide;
- a live mini-window preview driven by the actual motion theme.

All values are written to `SETTINGS.CNF` through configd and restored at boot.
