# Accounts and Authentication

Aster OS 0.8.0 adds the identity core for the first-run and sign-in experience.
It is **authentication and UX**, not a kernel-enforced security boundary. The
desktop remains one ring-3 process until AFS v2 and file permissions land in
0.8.1.

## 1. Account record

The shared `aster-auth` crate defines `ASTER-ACCOUNTS 1`:

```
username<TAB>display name<TAB>uid<TAB>gid<TAB>admin<TAB>home<TAB>salt<TAB>hash<TAB>iterations<TAB>created<TAB>last_login
```

- Usernames: 3-32 ASCII letters/digits/`.`/`_`/`-`, must not start with `.` or
  `-`, case-insensitive unique. `root`, `admin`, `system`, `configd`, `desk`,
  `setup` and other system names are reserved.
- UIDs start at 1000; GID defaults to the UID. `root` (0) is reserved.
- The home path is stored per account; the desktop uses `/home/U<uid>` when the
  FAT32 layout supports it, otherwise `/home/user/U<uid>`.
- `admin` is a flag, not a kernel capability.
- Salt is 16 bytes, hash 32 bytes, PBKDF2 iterations 100,000. The hash and salt
  are lowercase hex in the record. No plaintext password is stored.

## 2. Password hashing

`shared/aster-auth` implements SHA-256 (FIPS 180-4), HMAC-SHA256 (RFC 2104) and
PBKDF2-HMAC-SHA256 (RFC 8018) directly, without external crates. Published test
vectors are in the crate tests:

- SHA-256: `abc`, empty input, the 56-byte FIPS block.
- HMAC-SHA256: RFC 4231 cases 1 and 2.
- PBKDF2-SHA256: `password`/`salt` at 1, 2 and 4096 iterations.

Comparison of derived hashes is constant-time. Verification recomputes the
hash with the stored salt and iteration count.

## 3. Randomness and threat boundaries

0.8.0 does **not** yet ship a CSPRNG. When the account DB is integrated, salt
generation must use RDRAND/RDSEED plus a TSC mix, with the limitation recorded
here. A reviewed CSPRNG, TLS, signatures and secure boot are 0.8.3.

There is no login, lock screen or kernel permission enforcement in this phase.
Do not claim multi-user isolation, file privacy or privilege separation for
0.8.0.

## 4. Current integration status

Implemented and host-tested:

- username/full-name/password validation;
- PBKDF2 password creation and verification;
- account DB serialization, parsing, lookup, add/remove and UID allocation.

Not yet implemented:

- configd-owned account DB file and atomic write/backup;
- first-boot wizard and on-disk setup flag;
- login/lock/logout surfaces;
- system/user settings split, per-user homes, language/keyboard/timezone;
- Salt generation from a real entropy source.

These are tracked in `docs/releases/0.8.0/KNOWN_LIMITATIONS.md`. The account core
is real; the OS-level flows are not claimed until integrated and tested.
