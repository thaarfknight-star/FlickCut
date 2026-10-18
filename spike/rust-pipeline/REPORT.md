# FlickCut Tauri v2 — Video Pipeline Spike Report

**Author:** TAgent3 · **Date:** 2026-10-18 · **Status:** spike (research + prototype)

## Recommendation

**Use a bundled ffmpeg sidecar. Do not go native Rust for encoding in v1.**

Rationale, per effect:

| Effect | What it needs | ffmpeg sidecar | Native Rust |
|---|---|---|---|
| Slow-mo | Decode H.264 → resample frames → encode | ✅ proven (our Python pipeline does exactly this) | ❌ no practical pure-Rust H.264 decoder |
| Speed curve | Same as slow-mo, variable resampling | ✅ | ❌ same decode blocker |
| Smooth zoom | Crop/scale per frame (Rust `image` crate handles this natively) | ✅ | ⚠️ crop is easy; encode is the blocker |
| MP4 output | H.264 + MP4 mux | ✅ libx264, battle-tested | ⚠️ `openh264` + `mp4` crates work but no HW accel, more glue |

The decode side is the deciding factor: FlickCut must decode arbitrary
H.264 input (screen recordings). There is no production-ready pure-Rust
H.264 decoder. `openh264` (BSD-2, 0.9.x, 400k+ dl/month) can decode *and*
encode H.264 via Cisco's binary, but it is still a C++ library under the
hood, has no hardware acceleration, and we would still need MP4 muxing
(`mp4` crate) plus handling for non-H.264 inputs. That is a large surface
for v1 with no user-visible win over ffmpeg.

The ffmpeg sidecar mirrors our proven Python architecture 1:1:
Rust backend owns the region model + timeline math (this spike's prototype),
decodes/samples/crops frames, and pipes raw frames to the bundled ffmpeg
exactly like `editor.py` does today. The only thing that changes is the
language of the orchestrator.

Licensing note: our current pipeline uses libx264 (GPL). FlickCut is
AGPL-3.0, so a GPL ffmpeg build is license-compatible. If we ever need a
non-GPL ffmpeg, build it with `--enable-libopenh264` instead of libx264
(slightly worse compression, no GPL).

## Tauri v2 sidecar mechanics (verified against official docs)

- Binaries live in `src-tauri/binaries/`, named
  `ffmpeg-<target-triple>[.exe]` (e.g. `ffmpeg-x86_64-pc-windows-msvc.exe`).
  Tauri picks the current platform's binary automatically.
- Spawn from Rust: `app.shell().sidecar("ffmpeg")?.args([...]).spawn()?`
  (`tauri-plugin-shell`). From JS: `Command.sidecar('binaries/ffmpeg')`.
- Arg allowlisting goes in `src-tauri/capabilities/default.json`
  (`shell:allow-execute` + `sidecar: true` + arg validators) — required,
  otherwise the frontend cannot invoke it.
- Stdin piping (raw frames → ffmpeg) works from the Rust side via the
  spawned child's stdin handle; progress can be parsed from stderr
  (`-progress pipe:2`) and forwarded to the frontend as Tauri events —
  same pattern as our Python `export_progress` signal.

## Per-platform notes

- **Windows (x86_64-pc-windows-msvc):** primary target. Bundle a
  full-featured ffmpeg build (gyan.dev / BtbN). ~70MB added to the
  installer; acceptable. No system dependencies.
- **macOS (aarch64-apple-darwin + x86_64-apple-darwin):** needs two sidecar
  binaries (or one universal2 build renamed per triple). Unsigned ffmpeg
  inside a signed app: the *app* signature covers the bundle, but the
  sidecar binary must be signed with the same Developer ID or Gatekeeper
  will kill it on first run. For dev builds, `codesign --deep` or ad-hoc
  sign in CI. Notarization staples the whole bundle.
- **Linux (x86_64-unknown-linux-gnu):** static ffmpeg build (johnvansickle).
  AppImage/deb/rpm all fine; no special signing.

## Rust region-model design (prototype)

`flickcut-engine` crate (this spike, `flickcut-engine/`):

- `Effect` enum: `SlowMo | Curve | ZoomIn | ZoomOut`
- `Region { start, end, effect, params }` with `Param::{Float, Point, Points, Text}`
  (mirrors the Python dict-based params without dynamic typing pitfalls)
- `speed_at(t, regions)`, `zoom_at(t, regions)` — same semantics as
  `editor.py`, including the 0.35s smooth focus glide across zoom boundaries
  (ported from `_focus_center_at`)
- `set_region_intensity` / `region_intensity` / `describe_region` — same
  0..100 slider round-trip as the merged Python version
- 6 unit tests (round-trip all effects, clamping, describe formats,
  glide smoothness, single-region stability, slow-mo speed)

What the prototype deliberately excludes (Tauri integration work):
decoding, frame sampling/cropping (`image` crate), ffmpeg sidecar
spawning, Tauri commands/events, and the frontend. Those are mechanical
once this core is approved.

## Prototype status

`cargo test` could not run in this environment yet (Rust toolchain install
in progress — slow network). The code is written carefully against the
merged Python behavior; tests mirror the Python suite 1:1. If the toolchain
lands, `cargo test` in `flickcut-engine/` is the verification step.

## Open questions for DEVagent

1. ffmpeg build source per platform (gyan/BtbN/johnvansickle) — who vets them?
2. macOS signing story for the sidecar in CI (Developer ID available?).
3. Keep `editor.py` as reference implementation, or freeze it now?
