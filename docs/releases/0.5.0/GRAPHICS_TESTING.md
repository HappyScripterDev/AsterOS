# Aster OS 0.5.0 - Graphics Testing

## 1. Host tests

`cargo test -p aster-motion` runs eight unit tests:

- easing endpoints and monotonicity;
- tween completion across skipped frames;
- tween retarget;
- spring convergence and settling;
- motion track replacement/collection;
- owner cancellation;
- absolute-time sequence steps.

These run from `tools/test.py`.

## 2. QEMU acceptance test

`tools/graphics_test.py` boots a fresh data volume and drives the live desktop:

1. baseline screenshot;
2. open Terminal and Files;
3. minimize Files with Ctrl+M, wait for the animation, verify the old window
   area matches the wallpaper (no residue);
4. click the Files dock icon at its real coordinate, verify the window returns;
5. open Window Overview with F3, verify the desktop is dimmed and cards are
   drawn, then close;
6. hold Alt, tap Tab, verify the switcher panel and dim backdrop, release Alt;
7. switch to desktop 2 and verify desktop 1 windows are gone, then switch back;
8. open Text, type, create a second tab with Ctrl+T, type, select the first tab
   and cycle with Ctrl+PageDown; verify tab logs;
9. open Settings, move to Graphics & Motion (section 9), toggle Window
   animations off, and verify `motion=0,1,1,1,1` reached `/SETTINGS.CNF`;
10. close windows rapidly, switch desktops, open/close the overview and run
    `uname` in a fresh Terminal to prove the shell still responds.

The test uses real pixel checks (not just screenshots) for minimize/restore,
overview dimming, switcher visibility and workspace contents.

## 3. Regression suite

The 0.4.0 suites remain in place:

`test.py`, `ui_test.py`, `app_probe.py`, `pointer_test.py`, `wm_probe.py`,
`files_probe.py`, `fs_probe.py`, `persistence_test.py`, `storage_probe.py`,
`isolation_test.py`, `settings_test.py`, `service_test.py`, `final_ui_probe.py`,
`status_probe.py`, `shell_probe.py`, `about_probe.py`, `accent_probe.py`,
`net_test.py`, `network_ui_probe.py`.

`tools/test.py --ui` runs the UI test and the graphics test.

## 4. What is covered

- dirty-region repaint and no residue after minimize/close;
- open/close/minimize/restore/maximize/snap geometry transitions;
- dock click restore and dock setting persistence;
- overview live previews and dim layer;
- held-Alt switcher;
- workspace slide and window visibility;
- Text tabs and smooth scroll code path;
- menu/toast timing is exercised visually by the probes but not pixel-asserted;
- settings controls reach the persisted file.

## 5. Not covered

- No frame-by-frame visual diff or video recording.
- No forced low-FPS injection; animations are checked on the normal QEMU speed.
- No automated assertion for menu fade or toast fade beyond screenshots.
- No drag-reorder test (feature is not implemented).
- No leak or long-running soak test.
- No physical-hardware test; this is a software renderer only.
- No multi-monitor or runtime scale-factor test.
