# Aster OS 0.8.0 - Baseline Audit

Date: 2026-11-22
Branch: `milestone/aster-0.8.0-users`, from the 0.6 platform tip.
The 0.6 USB work is committed (WIP); it is not modified by this milestone.
`v0.5.0` is untouched and `v0.6.0` remains unset.

## 1. Build and test baseline

| Check | Result |
|---|---|
| `python tools/build.py` | PASS, image verified |
| `python tools/test.py` | PASS |
| `python tools/ui_test.py` | PASS |
| `python tools/app_probe.py` | PASS |
| `python tools/settings_test.py` | PASS |
| `python tools/persistence_test.py` | PASS |
| `python tools/files_probe.py` | PASS |
| `python tools/shell_probe.py` | PASS |
| `python tools/pointer_test.py` | PASS |
| `python tools/wm_probe.py` | PASS |

Boot to the desktop remains ~1.2-1.4 s under QEMU TCG. The desktop starts
directly as PID 1; there is no login or setup surface.

## 2. Settings and storage

- Persistent home: FAT32 at `/home/user` (8.3 names) on the data disk.
- System settings: `/home/user/SETTINGS.CNF`, owned by the ring-3 `configd`
  service. The desktop sends a compact v1 blob over IPC; configd validates it,
  writes `SETTINGS.NEW`, flushes, renames and keeps `SETTINGS.BAK`.
- Settings groups currently cover graphics/motion/dock/network; there is no
  system/user split, no language, keyboard layout or timezone key.
- `Settings > Users` is a placeholder: a single local account exists, no
  password, no lock screen.
- Several apps hardcode `/home/user` (Files sidebar, Terminal cwd, Text/Notes
  defaults, configd paths).

## 3. Input, font, clock

- PS/2 keyboard scancodes are translated to ASCII in `kernel/src/drivers/ps2.rs`
  with a US layout. There is no DE layout and no AltGr path.
- The font atlas contains ASCII 32-126 only; `äöüß` and `€` do not render.
- The RTC is read as local time (CMOS); QEMU currently uses
  `-rtc base=localtime`. There is no timezone ID, offset or DST rule.
- The clock, Calendar and Terminal `date` all use that raw RTC time.

## 4. Identity baseline

Before this milestone there was no account database, no password hashing, no
CSPRNG, no login, lock or logout, and no first-boot wizard.

## 5. Implemented in this phase

- New `shared/aster-auth` crate:
  - username rules (3-32, ASCII letters/digits/._-, reserved system names,
    case-insensitive uniqueness);
  - PBKDF2-HMAC-SHA256 from FIPS 180-4 / RFC 2104 / RFC 8018 with published
    SHA-256, HMAC-SHA256 (RFC 4231) and PBKDF2-SHA256 test vectors;
  - constant-time hash comparison;
  - versioned `ASTER-ACCOUNTS 1` record format with username, display name,
    UID/GID, admin flag, home, salt, hash, KDF iterations, created/last-login;
  - account add/remove/lookup/next-UID and round-trip parsing.
- `tools/accounts_test.py` runs the six host tests.

Not yet integrated: configd does not own the account DB, the desktop has no
setup/login/lock mode, the DB is not written to the FAT32 volume, and the
wizard, language/keyboard/timezone UI and per-user homes are absent. This is
documented as the exact 0.8.0 gap; no UI placeholder is added.

## 6. Gate

Build green, existing tests green, identity core host-tested, baseline
documented. The next phase is the on-disk account DB integration, followed by
the setup wizard and login/lock surfaces.
