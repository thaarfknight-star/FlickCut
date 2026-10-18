# flickcut-capture

Native screen capture module for the FlickCut Tauri v2 backend, built on
[`scap`](https://crates.io/crates/scap) (Windows Graphics Capture /
ScreenCaptureKit / PipeWire portal).

Implements the capture side of `docs/ipc-contract-v1.md`:

- `list_sources()` → `{ screens, windows }` (on Linux, scap delegates to the
  XDG portal picker, so a synthetic `"screen:portal"` source is offered)
- `Session::start(source_id, fps, output_path)` → session with a running
  encoder; owns the [`SessionClock`]
- `Session::stop()` → `{ video_path, duration_sec, frames_*, width, height }`

## Session clock

The capture module owns the session time base. Input telemetry (the
`flickcut-input` crate) syncs to it: clone [`SessionClock`] from
[`Session::clock`] and call `clock.stamp()` per input event — `t_mono`
(seconds since session start, monotonic) is directly comparable with video
frame timestamps.

## Pipeline

One worker thread runs the scap capture loop and pipes raw BGRA frames into
an `ffmpeg` subprocess (`libx264`, yuv420p, faststart) writing MP4. Stopping
is cooperative via an atomic flag; `stop()` joins with a timeout and kills
the encoder as a last resort, so callers never hang. scap panics (instead of
`Err`) when its backend cannot initialize (e.g. no D-Bus on Linux) — the
build is wrapped in `catch_unwind` and surfaced as `CaptureError`.

## Test harness

`cargo run --bin flickcut-harness -- list` /
`cargo run --bin flickcut-harness -- record <source_id> <seconds> <out.mp4>`
exercises the full pipeline without the Tauri UI.
