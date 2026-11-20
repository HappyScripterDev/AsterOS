# Security policy

Aster OS is a developer preview and has **not** undergone a third-party security
audit. Do not use it for sensitive data or as a production system.

The current security properties, adversary model, verified mitigations, and
known gaps are documented in
[docs/security/THREAT_MODEL.md](docs/security/THREAT_MODEL.md). In summary:

- Ring-3 isolation, per-process page tables, NX/W^X user mappings, and syscall
  pointer validation are implemented.
- There is no ASLR, SMEP/SMAP, sandboxing, user authentication, signed boot,
  signed packages, or secure update.
- All built-in apps share the desktop address space.

## Reporting a vulnerability

Do not open a public issue for a security problem. Contact the maintainers
privately (repository security advisory or the address listed in the project
profile) with:

- affected component and version/commit,
- a minimal reproduction (image, commands, serial log),
- impact and any known workaround.

We will acknowledge within a reasonable time, investigate, and publish a
coordinated fix. Because this is a research-preview project, no formal SLA is
offered yet.

## Scope

In scope: the bootloader, kernel, syscall ABI, filesystem, input stack, desktop,
packaging/update code when it exists, and the build tooling's effect on the OS
image.

Out of scope for now: the host OS, QEMU, OVMF, and other build dependencies,
except where a configuration choice weakens the tested system (for example,
shipping an insecure firmware image). Report those upstream as well.
