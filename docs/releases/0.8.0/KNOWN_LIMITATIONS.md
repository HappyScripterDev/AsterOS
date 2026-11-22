# Aster OS 0.8.0 - Known Limitations

Status: **not release-complete**. The account/identity core is implemented and
host-tested; the first-boot wizard, login, lock, logout, localization,
keyboard layouts, timezones and per-user integration are not yet implemented.
No UI placeholder or fake security claim is present.

## Implemented

- `shared/aster-auth`: username/full-name/password rules, PBKDF2-HMAC-SHA256
  with published test vectors, constant-time verification, versioned account
  records and UID allocation.
- `tools/accounts_test.py` (six host tests).

## Missing (exact)

- **First-boot wizard.** No setup mode, setup_done flag or wizard UI exists. A
  fresh image still boots directly to the desktop.
- **Login, lock and logout.** There is no login screen, no `Super+L` lock, no
  logout flow and no retained/reset session UX.
- **Account persistence.** Configd does not own `ACCOUNTS.CNF`; the account DB
  is not written to the FAT32 volume, so accounts do not survive a reboot yet.
- **Salt entropy.** No RDRAND/RDSEED/TSC salt generator is wired. The KDF and
  record format are ready; salt generation must be added before any account is
  created on a real image.
- **Per-user homes/settings.** `/home/user` remains the single home. Existing
  paths are still hardcoded in Files, Terminal, Text, Notes and configd.
- **Language/i18n.** English only; no German string table. The font atlas is
  ASCII 32-126 and cannot render `äöüß` or `€`.
- **Keyboard layouts.** PS/2 still maps US scancodes only; there is no DE or
  AltGr path and no shared layout module.
- **Timezone.** RTC is treated as local time; there is no zone list, UTC offset,
  DST rule or `-rtc base=utc` run mode. Clock/Calendar/`date` do not use a zone.
- **Settings pages.** `Settings > Users` remains a placeholder; Language &
  Region is absent.
- **Tests.** `setup_test.py`, `login_test.py`, `lock_test.py`,
  `i18n_test.py`, `keyboard_test.py`, `timezone_test.py` and `users_probe.py`
  are absent because their features are absent. `accounts_test.py` covers only
  the host identity core.

## Not claimed

No kernel-enforced multi-user isolation, file permissions, sandboxing, CSPRNG,
TLS or secure boot is claimed. Those are 0.8.1/0.8.3. The desktop remains one
process. The `v0.8.0` tag must not be set while the list above is open.
