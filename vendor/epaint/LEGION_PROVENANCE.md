# Vendored epaint provenance

This directory is based on the crates.io package `epaint 0.34.2`, from the
local Cargo registry archive:

`C:\Users\dasbl\.cargo\registry\cache\index.crates.io-1949cf8c6b5b557f\epaint-0.34.2.crate`

Upstream repository: https://github.com/emilk/egui/tree/0.34.2/crates/epaint
Package license declaration: `MIT OR Apache-2.0`
Archive size: 109,515 bytes
Archive SHA-256: `92b452e348c2758115288802ca25f86ee286ce2cfae6643711ce116662311310`

The archive contains 42 regular members: `.cargo_vcs_info.json`, `Cargo.lock`,
`Cargo.toml`, `Cargo.toml.orig`, `README.md`, `benches/benchmark.rs`, and the
source tree under `src/` (`brush.rs`, `color.rs`, `corner_radius.rs`,
`corner_radius_f32.rs`, `direction.rs`, `image.rs`, `lib.rs`, `margin.rs`,
`margin_f32.rs`, `mesh.rs`, `mutex.rs`, `shadow.rs`, `shape_transform.rs`,
`shapes/{bezier_shape,circle_shape,ellipse_shape,mod,paint_callback,path_shape,rect_shape,shape,text_shape}.rs`,
`stats.rs`, `stroke.rs`, `tessellator.rs`, `text/{cursor,font,fonts,mod,text_layout,text_layout_types}.rs`,
`texture_atlas.rs`, `texture_handle.rs`, `textures.rs`, `util/mod.rs`, and
`viewport.rs`). No license file is present in the archive.

## Current patch and hashes

The active Legion patch changes exactly these source files relative to the
archive:

* `src/text/fonts.rs` — exposes bounded unwrapped chunk layout and retains a
  per-fonts-instance layout identity. SHA-256: `2cdde05a5c62165f9ff507db4ecc1728660b3a255c9a98250251be0b86edf91d`.
* `src/text/text_layout.rs` — adds bounded continuation state, shared shaping,
  validation, and regression tests. SHA-256: `0181a3b586d7674d190f8d1be5f8d469d30b2f10dfe7c2e71027efd82fa0aa75`.

The upstream SHA-256 values for those same files are:

* `src/text/fonts.rs`: `154b2d3bacdcdbc582b3727a764bcfadfb12378095a0b73819fefc91f90a2313`
* `src/text/text_layout.rs`: `e3fc0cf86c98a68d2825020faa098224e7fc100d33cc91a80ed9c2003dd1b0f2`

`Cargo.lock` is retained as an aligned standalone lock for reproducible vendor
tests. It is not a source patch. Current SHA-256:
`e878979440c267e1ebf648e0fc8d3e390a85c7db2b26b6205922c63e0cc3bbfc`.

## Verification evidence

Using the bundled Python runtime at
`C:\Users\dasbl\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe`,
the archive member inventory and hashes above were compared against this
directory. The standalone command
`cargo test --manifest-path vendor/epaint/Cargo.toml --lib` passed: 40 passed,
0 failed, with the aligned lock. The captured output is retained in the plan
scratch file `s1-04h-vendor-fix-tests.log`.

Root integration evidence is retained in scratch: desktop targeted tests passed
in `s1-04h-desktop-final-checks.log`, and `cargo deny check` passed with
advisories, bans, licenses, and sources all okay in
`s1-04h-integration-final-gates.log`. These are verification records only; this
file makes no full wrapped or production qualification claim.

Canonical upstream license texts are included beside this notice:

* [`LICENSE-MIT`](https://raw.githubusercontent.com/emilk/egui/0.34.2/LICENSE-MIT),
  SHA-256 `95ca92f5f8ea5231f1580b3a2a799e8260af3114b900e1def5355a7f44bcf60c`.
* [`LICENSE-APACHE`](https://raw.githubusercontent.com/emilk/egui/0.34.2/LICENSE-APACHE),
  SHA-256 `8173d5c29b4f956d532781d2b86e4e30f83e6b7878dce18c919451d6ba707c90`.
