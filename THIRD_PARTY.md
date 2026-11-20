# Third-party components

Aster OS does not include Linux, Windows, macOS, GNU, or other operating
system code. It does use the following build-time or runtime components.

## Runtime / linked

| Component | Version | License | Use |
|---|---|---|---|
| `libm` | 0.2.x | MIT OR Apache-2.0 | `no_std` floating point helpers in the ring-3 desktop (font metrics, animation easing, procedural wallpapers). |
| Inter (font) | variable, OFL 1.1 | SIL Open Font License 1.1 | Rasterized into `assets/fonts/atlas.bin` at build time by `tools/fontgen.py`. The OS ships only the bitmap atlas. |
| JetBrains Mono (font) | variable, OFL 1.1 | SIL Open Font License 1.1 | Rasterized monospaced face for Terminal and Text. |

The font files themselves are downloaded by `tools/fontgen.py` and are not
committed. The generated atlas is derived data permitted by the OFL.

## Build and test only (not part of the OS image)

| Component | License | Use |
|---|---|---|
| Rust / `rust-lld` / rustup | MIT OR Apache-2.0 | Compiler, linker, core/alloc. |
| QEMU | GPL-2.0 | Emulator used for testing. |
| OVMF / EDK II | BSD-2-Clause-Patent | UEFI firmware for QEMU, fetched by `tools/fetch_firmware.py`. Not redistributed. |
| Pillow | HPND / MIT-CMU | Host font atlas generation. |
| Python standard library | PSF | Build, image, and test tooling. |

## Design inspiration

Aster's visual language is informed by general, published interaction
principles (hierarchy, spacing, translucency, purposeful motion). No Apple,
Microsoft, or third-party icons, artwork, sounds, or layout files are copied.
All icons in the desktop are drawn procedurally in `user/asterdesk/src/ui.rs`.
The wallpapers are generated procedurally in `theme.rs`.
