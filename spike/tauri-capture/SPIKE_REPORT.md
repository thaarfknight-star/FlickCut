# FlickCut Tauri v2 — Screen-Capture Feasibility Spike

**Author:** TAgent (research-grade spike, 2026-10-18)
**Question:** which native capture API per platform works inside Tauri v2?
**Short answer:** all three work — via three different Rust crates, called from
Tauri backend code on background threads. There is no maintained unified
cross-platform crate; use one best-in-class crate per platform.

## Recommendation

| Platform | Native API | Rust crate | Version (checked 2026-10-18) | License | Maintenance |
|---|---|---|---|---|---|
| Windows | Windows Graphics Capture | `windows-capture` | 2.0.1 (2026-08-08) | MIT | active |
| macOS | ScreenCaptureKit | `screencapturekit` | 11.0.0 (2026-09-24) | MIT/Apache-2.0 | very active (~1M dl/mo) |
| Linux | PipeWire + xdg-desktop-portal | portal/`pipewire` stack | — | MIT/Apache-2.0 | active (freedesktop) |

All three licenses are AGPL-3.0-compatible as dependencies.

## Why not one crate for everything

- **`scap`** (Cap team's original unified crate): effectively unmaintained —
  last release 0.1.0-beta.1 (2025-08-04), PipeWire 0.8.0 dependency chain
  breaks compilation on current distros. Do not base a new 2026 project on it.
- **`xcap`**: 2.4M downloads, but screenshot-oriented; video recording is WIP
  and the frame model is thin (no stride/format/timestamps). Fine for
  thumbnails, not for a recorder.
- **`pinray`** (0.2.7, Oct 2026): clean unified frame model, capture-only, no
  ffmpeg dependency — promising but 1k downloads; unproven. Revisit in 6 months.

## Per-platform notes

### Windows — Windows Graphics Capture (`windows-capture` 2.x)
- The modern API (Win10 1803+); per-monitor and per-window capture, cursor/
  border/dirty-region options. The crate also exposes DXGI Desktop Duplication
  and a hardware-accelerated encoder.
- No special permission needed for normal windows. Capturing an elevated
  (admin) game while FlickCut runs unelevated fails — installer should keep
  the existing `PrivilegesRequired=admin` default or document it.
- Runs on any thread; in Tauri, drive it from a dedicated capture thread and
  forward control (start/stop/target) via Tauri commands.

### macOS — ScreenCaptureKit (`screencapturekit` 11.x)
- The only Apple-blessed API (macOS 13.0+); zero-copy `IOSurface` frames,
  system-audio capture (13.0+), screenshots, content picker (14.0+).
- **Blocker to design around:** the OS shows a Screen Recording permission
  prompt on first use; capture fails until the user grants it in System
  Settings. The app must detect denial and guide the user (the crate exposes
  permission-check helpers).
- Async API works with any executor; use `tauri::async_runtime` or Tokio.

### Linux — PipeWire + xdg-desktop-portal
- The correct modern path on both Wayland and X11. Flow: request a screencast
  session over D-Bus → the compositor shows a **source picker** → PipeWire
  streams frames.
- **Blocker to design around:** the picker is a user-consent step every fresh
  session (tokens can persist the choice per app). A "click Record and go"
  flow needs UX for this (remember choice, re-request silently when possible).
- Needs a running PipeWire session and `xdg-desktop-portal` (+ backend for
  the DE). On headless/CI there is nothing to capture — tests must mock.

## How it fits inside Tauri v2

Tauri v2 plugins/commands are plain Rust, so every crate above works
directly in the backend:
- Capture loop lives on a dedicated Rust thread (never the webview thread).
- **Do not ship raw frames to the frontend.** Encode in Rust (bundled
  `ffmpeg` sidecar via `tauri-plugin-shell`, or the `ffmpeg-next` crate)
  and send only control events + thumbnails over IPC.
- Per-frame timestamps (wall + monotonic) are produced at grab time in Rust —
  the same contract as the Python recorder's sidecar JSON, so the moment/
  edit pipeline ports 1:1.
- Precedent: **Cap** (the open-source Loom alternative this project is modeled
  on) is itself a Tauri + Rust app doing exactly this.

## Prototype in this repo

`spike/tauri-capture/flickcut-capture-spike/` — minimal Rust binary implementing
the unified `Frame { width, height, t_wall, t_mono, pixels_rgba }` +
`CaptureTarget` model, with the Linux backend (`xcap` screenshots) compiled
and exercised.

**Verification levels (honest):**
- **Build: PASS.** `cargo build` clean on Rust 1.89 (Linux x86_64). Full
  dependency tree resolves, including the PipeWire sys crates.
- **Build-time system deps (CI implication):** xcap pulls PipeWire/Wayland
  native deps unconditionally on Linux. CI runners need:
  `libpipewire-0.3-dev libclang-dev libgbm-dev libdrm-dev libegl-dev
  libwayland-dev libxcb1-dev libxcb-shm0-dev libxcb-randr0-dev` (+ pkg-config).
- **Target enumeration: PASS.** Against Xvfb (1280x800), `list` returns the
  monitor with correct geometry.
- **Pixel capture: PARTIAL.** `capture_image` fails under this sandbox's Xvfb
  ("Connection error" from x11rb — likely an XShm quirk of the virtual
  server, not of xcap itself). On headless hosts without X, both commands
  exit gracefully with clear errors (exit 1). Live pixel capture still needs
  verification on a real desktop session.
- Windows (`windows-capture`) / macOS (`screencapturekit`) backends: code
  sketched against docs.rs APIs, **not compiled** — no Windows/macOS target
  available on this host. Must be compile-verified in CI on those runners
  before being trusted.

## Risks / open questions for the build phase

1. Linux portal picker UX vs. "one-click Record" — needs a design decision.
2. macOS Screen Recording permission onboarding flow.
3. Encoding story: bundled ffmpeg sidecar (simplest, bigger installer) vs.
   `ffmpeg-next` (native, harder CI) vs. `windows-capture`'s built-in HW
   encoder on Windows only.
4. Cross-compilation CI: `cargo check` per target on native runners
   (windows-latest, macos-latest) — no cross toolchain needed.
5. `scap` should be revisited only if it gets a maintained fork.
