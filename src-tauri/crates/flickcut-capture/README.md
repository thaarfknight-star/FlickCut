# flickcut-capture

Native per-platform screen capture for the FlickCut Tauri v2 backend.

| Platform | API | Crate |
|---|---|---|
| Windows | Windows Graphics Capture | `windows-capture` 2.x |
| macOS | ScreenCaptureKit | `screencapturekit` 11.x |
| Linux | XDG portal ScreenCast + PipeWire | `ashpd` 0.10 + `pipewire` 0.8 |

Implements the capture side of `docs/ipc-contract-v1.md`:

- `list_sources()` → `{ screens, windows }` (on Linux the portal owns
  enumeration, so a synthetic `"screen:portal"` source is offered and the OS
  picker appears at capture start)
- `Session::start(source_id, fps, output_path)` → session with a running
  encoder; owns the [`SessionClock`]
- `Session::stop()` → `{ video_path, duration_sec, frames_*, width, height }`

## Architecture

`backends::Backend` trait — each platform pushes unified `Frame`s
(BGRA, row-major, no padding) into a channel; one worker thread drains it
into the bundled `ffmpeg` sidecar (`libx264`, yuv420p, faststart) writing MP4.
Stopping is cooperative: backend stop → channel close → worker join (timeout)
→ encoder kill fallback. Callers never hang.

## Session clock

The capture module owns the session time base. Input telemetry (the
`flickcut-input` crate) syncs to it: clone [`SessionClock`] from
[`Session::clock`] and call `clock.stamp()` per input event — `t_mono`
(seconds since session start, monotonic) is directly comparable with video
frame timestamps.

## Test harness

`cargo run --bin flickcut-harness -- list` /
`cargo run --bin flickcut-harness -- record <source_id> <seconds> <out.mp4>`
exercises the full pipeline without the Tauri UI.
