# libspa 0.8.0 — vendored with a FlickCut patch

Upstream: https://github.com/pipewire-rs/pipewire-rs (MIT, see LICENSE)

## Why vendored

`libspa` 0.8.0 does not compile against PipeWire < 0.3.6x system headers:
`VideoInfoRaw::new()` uses a struct literal naming `flags`/`modifier`/
`multiview_flags`, which don't exist in `spa_video_info_raw` on e.g.
Ubuntu 22.04 (PipeWire 0.3.48). The crate builds fine on newer systems,
so this only breaks CI.

## The patch

`src/param/video/raw.rs` — `VideoInfoRaw::new()` now constructs the struct
field-by-field over `mem::zeroed()` instead of a struct literal, so it
compiles against both old and new headers. Behavior is identical: the new
fields stay zero, exactly as the original literal set them to 0.

Wired via `[patch.crates-io]` in `src-tauri/Cargo.toml`.
