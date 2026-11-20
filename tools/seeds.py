"""First-boot contents of the persistent home volume.

Shared by the image builder, the persistence test and the benchmark so all
three start from the same known-good state.
"""

from __future__ import annotations

SEED_FILES: dict[str, bytes] = {
    "/WELCOME.TXT": (
        b"Welcome to Aster OS.\n\n"
        b"This home directory lives on the persistent data volume.\n"
        b"Files here survive a reboot.\n"
    ),
    "/SETTINGS.CNF": (
        b"version=1\naccent=blue\nanimations=1.00\n"
        b"wallpaper=0\nreduce_motion=0\nbrightness=1.00\n"
    ),
    "/NOTES/IDEAS.TXT": (
        b"Ideas\n-----\n"
        b"- Native compositor with real window management\n"
        b"- Persistent home volume\n"
        b"- Process isolation and IPC\n"
    ),
    "/DOCS/README.TXT": b"Documents saved here persist across reboots.\n",
}
