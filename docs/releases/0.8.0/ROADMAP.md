# Aster OS 0.8.0 - Users, Login & First-Boot Setup

Status: planned. Baseline: 0.6.0 platform work (untagged, 2026-11-22).

## Product direction

- 0.8.0 delivers the first-run and sign-in experience: accounts, login, lock,
  logout, a setup wizard and per-user homes and settings.
- The setup wizard asks for language, keyboard layout and timezone before it
  creates the first account, similar to a Linux installer or a desktop OOBE.
- This is authentication and UX, not yet a kernel-enforced multi-user security
  boundary. The desktop remains one ring-3 process. AFS v2 and kernel-enforced
  file permissions follow in 0.8.1. Do not claim multi-user security in 0.8.0.
- The UI remains the light glass desktop from 0.5. No visual rewrite.

## Goals

1. A versioned account database with usernames, UIDs, password hashes, admin
   flags and per-user homes.
2. A first-boot wizard: language, keyboard layout, timezone, account creation
   and finish.
3. Login, lock and logout surfaces with real password checks.
4. English and German localization for the shell, onboarding, login and
   Settings, including Latin-1 glyph coverage.
5. US and DE keyboard layouts, applied immediately in the wizard and persisted.
6. A curated timezone list with UTC offsets and DST, used by the clock,
   Calendar and `date`.
7. Per-user settings and home directories, with migration of existing
   `/home/user` data.
8. Tests, documentation and release evidence.

## Non-goals

- AFS v2, kernel-enforced UID/GID permissions or file isolation (0.8.1).
- Sandboxing, capabilities, ASLR/KASLR, SMEP/SMAP/UMIP, quotas and audit log
  (0.8.3).
- CSPRNG, TLS, certificates, signatures, secure boot and firewall (0.8.3).
- Package manager, Store, SDK and out-of-process apps (0.8.2).
- Full translations of every application; the shell/system surfaces are
  localized first and app strings are documented.
- Concurrent fast user switching. One active session; logout before another
  user logs in.
- Biometrics, avatars beyond generated initials, remote login.

## Starting point

- `milestone/aster-0.6.0-platform` is the current tip. USB work may be
  uncommitted; do not touch or revert it. The 0.6.0 tag is not set.
- 0.7.0 AFS is planned but not implemented; 0.8.0 does not depend on it.
- `Settings > Users` is a placeholder: "A single local account exists; there is
  no password, lock screen or privilege escalation yet."
- Settings live at `/home/user/SETTINGS.CNF`, owned by `configd`.
- The FAT32 home is mounted at `/home/user` with 8.3 names.
- The font atlas covers ASCII 32-126 only; German umlauts do not render.
- PS/2 scancodes are translated to ASCII in the kernel with a US layout.
- The clock uses the RTC as local time; there is no timezone setting.
- The desktop starts directly as PID 1; there is no login or setup screen.
- Built-in apps hardcode `/home/user` in several places.

## Phase 0 - Audit and baseline

- Create `milestone/aster-0.8.0-users` from the 0.6 tip after the USB work is
  committed or explicitly stashed; do not commit the user's in-progress work.
- Build and run the full suite.
- Record the current settings path, `configd` protocol, user placeholder, input
  path, RTC/time code, font coverage and hardcoded home paths.
- Measure baseline: boot to desktop, settings load, font atlas size, login
  absent, keyboard mapping.
- Write `BASELINE.md`.

Gate: build green, existing tests green, baseline documented.

## Phase 1 - Account and identity core

- Define a versioned account database with atomic writes and a backup:
  username, full name, UID, GID, password hash, salt, KDF parameters, admin
  flag, home path, created and last login.
- Usernames: 3-32 characters, letters/digits/`.`/`_`/`-`, case-insensitive
  unique; reserve system names. Full name is display-only.
- UIDs start at 1000; root (0) is reserved. Home paths are `/home/U<uid>` so
  they stay 8.3-safe on FAT32.
- Password hashing: use a reviewed Argon2id implementation if a `no_std`
  compatible crate builds; otherwise PBKDF2-HMAC-SHA256 with a documented
  iteration count and test vectors. Never store plaintext. Do not invent a
  hash.
- If no CSPRNG exists, add a minimal kernel RNG seeded from RDRAND/RDSEED with
  a TSC/HPET mix and document its limits. Full CSPRNG/TLS is 0.8.3.
- Add account load/save, validation, UID allocation and password verification.
- Gate: create two accounts, verify correct and incorrect passwords, persist
  across reboot, and confirm no plaintext password is stored.

## Phase 2 - First-boot setup wizard

- Detect first boot through a system settings file or a `setup_done` flag.
- Wizard steps:
  1. Welcome and language selection (English/Deutsch).
  2. Keyboard layout (US/DE at minimum) with a live test field.
  3. Timezone selection with a city list and current local time preview.
  4. Account creation: full name, username, password, confirmation, admin flag.
  5. Summary and finish.
- The wizard is keyboard-navigable with visible focus, back/next, validation
  and plain error messages. It uses the selected language immediately.
- On finish: write system settings, create the admin account, create
  `/home/U<uid>`, migrate existing `/home/user` data into it, keep the old
  directory as a backup, mark setup complete and start the session.
- If migration is not possible, stop with a clear error instead of losing data.
- Gate: a fresh data image boots into the wizard, completes it, reboots into
  the login screen, and the existing `/home/user` files are readable in the new
  home.

## Phase 3 - Login, lock and logout

- Login screen: account list or username field, password field, error message,
  keyboard layout and timezone from system settings, shutdown/restart actions.
- Lock screen: `Super+L` and an app-menu item; password unlock; session state
  preserved; wrong password shows an error without dismissing the lock.
- Logout: close the session, return to login and clear per-session UI state.
  Warn if an app has unsaved changes, or document that save is explicit.
- Failed login delay after repeated failures (for example 1 s after three
  failures); no permanent lockout.
- The login/lock surfaces never expose the password in logs or the account DB.
- Gate: login with correct and incorrect passwords, lock/unlock, logout and
  login again; the lock screen cannot be dismissed by clicking or Escape.

## Phase 4 - Language, keyboard and timezone

- Add a `Locale` (en-US, de-DE) and a string table for the shell, onboarding,
  login, lock, Settings and common dialogs. Fall back to English for missing
  keys. No hardcoded user-facing strings in those surfaces.
- Extend the font atlas to Latin-1 (at least U+00C0-U+00FF) including äöüß and
  common punctuation; update `tools/fontgen.py` and glyph lookup. Missing
  glyphs still render as a box.
- Keyboard layouts: US and DE. Move printable translation into a shared layout
  module used by PS/2 and USB HID; the active layout is set through a syscall or
  a kernel setting before login. DE includes AltGr for `@`, `€`, `{`, `}` and
  `\`.
- Timezone: a curated table of IANA-like zones with current UTC offset and DST
  rules (UTC, Europe/London, Europe/Berlin, Europe/Paris, Europe/Moscow,
  America/New_York, America/Chicago, America/Denver, America/Los_Angeles,
  America/Sao_Paulo, Asia/Dubai, Asia/Kolkata, Asia/Shanghai, Asia/Tokyo,
  Asia/Singapore, Australia/Sydney, Pacific/Auckland). Store the zone id.
- Treat the RTC as UTC and apply the selected offset; update `tools/run.py` to
  `-rtc base=utc` and document the change. Use the zone in the top-bar clock,
  Calendar, `date` and Settings preview.
- Gate: switch language, keyboard and timezone in setup and Settings; settings
  persist; German umlauts render; DE produces äöüß/@/€; the clock shows the
  correct local time.

## Phase 5 - Settings and per-user data

- Move the FAT32 mount to `/home` (or add a system area) so `/home/SYSTEM.CNF`
  and `/home/U<uid>/SETTINGS.CNF` can coexist. Migrate `/home/user` contents
  into `/home/U1000` and keep a backup.
- Centralize paths in a `paths` module: current home, system settings, user
  settings, documents, notes. Remove hardcoded `/home/user` from apps.
- Extend `configd` to serve system settings and per-user settings. The shell
  loads the active user's settings after login; system settings (language,
  keyboard, timezone) are shared.
- Settings > Users: current user, full name, password change, add/remove user
  (admin only), admin flag and last login.
- Settings > Language & Region: language, keyboard layout, timezone, date/time
  format. Apply immediately and persist.
- Files sidebar, Terminal `cd ~`, Text and Notes defaults use the current
  user's home.
- Gate: two users have separate homes and settings; password change works;
  add/remove user works; reboot preserves everything.

## Phase 6 - Tests, documentation and release evidence

- New tests:
  - `tools/setup_test.py`: fresh-image wizard, language/keyboard/timezone,
    account creation and first login.
  - `tools/login_test.py`: correct/incorrect password, logout, retry delay.
  - `tools/lock_test.py`: lock, wrong password, unlock and no bypass.
  - `tools/accounts_test.py`: account DB, UIDs, password change, add/remove.
  - `tools/i18n_test.py`: English/German strings and Latin-1 glyphs.
  - `tools/keyboard_test.py`: US/DE layouts and AltGr characters.
  - `tools/timezone_test.py`: zone list, offset and clock preview.
  - `tools/users_probe.py`: Settings > Users and Language & Region.
- Tests must use a disposable data image; never delete the user's
  `build/data.img`.
- Update `ui_test.py`, `app_probe.py`, `settings_test.py`,
  `persistence_test.py`, `files_probe.py`, `shell_probe.py`,
  `pointer_test.py` and `wm_probe.py`.
- Documentation: `docs/design/ONBOARDING.md`, `LOGIN.md`, `I18N.md`,
  `KEYBOARD.md`, `TIMEZONE.md`, `USERS.md`, plus updates to `DESKTOP_SPEC.md`,
  `INTERACTION.md`, `DESIGN_SYSTEM.md`, `docs/FEATURES.md`,
  `docs/PROJECT_STATUS.md`, `docs/ROADMAP.md`, `README.md`, `CHANGELOG.md` and
  `docs/releases/0.8.0/`.
- Benchmarks: setup time, login time, boot to login, settings load and font
  atlas size.
- Release files: `BASELINE.md`, `TEST_REPORT.md`, `PERFORMANCE.md`,
  `KNOWN_LIMITATIONS.md` and `CHANGELOG.md`.

Gate: every test green, no regressions, docs accurate, no fake security claims.

## Acceptance

| Area | Criterion |
|---|---|
| Build | `python tools/build.py` passes and the image verifies |
| First boot | Fresh image opens the setup wizard, not the desktop |
| Setup | Language, keyboard layout, timezone and account are configured |
| Accounts | Account DB persists; passwords are hashed, never plaintext |
| Login | Correct password enters the session; wrong password is denied |
| Lock | `Super+L` locks; password unlocks; no click/Escape bypass |
| Logout | Returns to login and clears session UI |
| Language | English/German shell, onboarding, login and Settings; umlauts render |
| Keyboard | US/DE layouts work; DE AltGr characters work |
| Timezone | Zone persists; clock/Calendar/`date` show local time |
| Homes | Per-user `/home/U<uid>`; existing `/home/user` migrated with backup |
| Settings | Per-user settings load after login; system settings shared |
| Honesty | No claim of kernel-enforced multi-user isolation in 0.8.0 |
| Tests | Existing and new tests green |
| Docs | Design, feature, status, roadmap, README and release docs updated |

## Risks

| Risk | Mitigation |
|---|---|
| Password hashing without a CSPRNG | Use a vetted KDF, add a minimal RNG with documented limits, never store plaintext |
| Font atlas growth | Extend to Latin-1 only, measure image size, keep missing-glyph boxes |
| Keyboard layout ABI churn | Shared layout module for PS/2 and USB HID, default US, apply before login |
| Timezone scope | Curated zone table with current offsets/DST; document historical limits |
| Home path migration loses data | Copy, verify, keep the old directory, stop on mismatch |
| Settings service complexity | System settings and per-user settings separated; direct-I/O fallback remains |
| Security overclaim | Document that 0.8.0 is authentication/UX, not isolation |
| Uncommitted 0.6 USB work | Do not touch or revert; branch only after it is committed or stashed |

## File map

- `user/asterdesk/src/accounts.rs` - account DB, password hashing, UID allocation.
- `user/asterdesk/src/session.rs` - setup, login, lock and logout state.
- `user/asterdesk/src/i18n.rs` - locale and string tables.
- `user/asterdesk/src/keymap.rs` - US/DE layouts and AltGr.
- `user/asterdesk/src/timezone.rs` - curated zones, offsets and DST.
- `user/asterdesk/src/paths.rs` - system and per-user paths.
- `user/asterdesk/src/desktop.rs` - setup/login/lock surfaces and menu actions.
- `user/asterdesk/src/apps.rs` - Settings > Users, Language & Region, Date & Time.
- `user/asterdesk/src/settings.rs`, `services.rs` - system/per-user settings.
- `user/configd/src/main.rs` - settings service paths.
- `kernel/src/drivers/ps2.rs` - layout hook.
- `kernel/src/drivers/xhci.rs` - layout hook when USB lands.
- `kernel/src/syscall.rs`, `shared/aster-abi/src/lib.rs` - layout, RNG and
  account-related syscalls as needed.
- `tools/fontgen.py`, `user/asterdesk/src/font_data.rs` - Latin-1 atlas.
- `tools/setup_test.py`, `login_test.py`, `lock_test.py`, `accounts_test.py`,
  `i18n_test.py`, `keyboard_test.py`, `timezone_test.py`, `users_probe.py`.
- `docs/design/ONBOARDING.md`, `LOGIN.md`, `I18N.md`, `KEYBOARD.md`,
  `TIMEZONE.md`, `USERS.md`.
