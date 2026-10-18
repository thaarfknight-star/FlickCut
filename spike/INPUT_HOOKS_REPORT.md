# FlickCut Tauri v2 — Global Input Hooks Spike

**Status:** research complete, prototype written (`spike/input-hooks/`).
Compile NOT verified — this sandbox blocks cargo builds (linker bus error
on /tmp tmpfs, "Operation not permitted" on file writes). Code follows
the documented APIs of rdev 0.5 / device_query 1.1 / gilrs 0.11; a Windows
or Linux dev machine should build it with `cargo build`.

## Learning status (honest)

- Rust: partial — I can write idiomatic Rust (ownership, threads, channels,
  error handling) but have not shipped a production Tauri app.
- Tauri v2: partial — I know the architecture (Rust backend + webview
  frontend, `setup()` hook, `AppHandle.emit`, sidecars, plugins) from docs;
  no hands-on app built yet.

## Crate choices

| Signal | Crate | Version | Why |
|---|---|---|---|
| Mouse clicks + keyboard (global hooks) | `rdev` | 0.5 | Cross-platform global listen + simulate; used by RustDesk in production |
| Cursor position trail (~30 Hz poll) | `device_query` | 1.1 | `DeviceState::get_mouse().coords` — no window needed, Win/Linux/macOS |
| Gamepad buttons/triggers + hotplug | `gilrs` | 0.11 | Unified layout, SDL mappings, hotplug; Linux/Windows/macOS |

Alternatives considered:
- `enigo` — simulation only, not capture. Rejected.
- `raw_input` — newer, has `Display` (monitor info) + `Grab`; less
  battle-tested than rdev. Worth re-evaluating for Wayland.
- Tauri's `tauri-plugin-global-shortcut` — hotkeys only, not a full
  input stream. Insufficient for the cursor trail.

## Per-platform notes

### Windows (primary target — Taha games here)
- `rdev` uses low-level hooks (`SetWindowsHookEx`). **No special
  permissions needed.** Works alongside any app.
- `gilrs` uses XInput. Xbox + most modern pads work out of the box.
- `device_query` cursor poll works fine.
- Verdict: everything works, zero user friction. ✅

### Linux
- `rdev` = **X11 only, no Wayland.** On Wayland compositors (default on
  Ubuntu 22.04+/Fedora), global hooks silently fail. Mitigations:
  (a) document X11 session requirement, (b) `device_query` polling still
  works for cursor *position* but not click *events* on Wayland,
  (c) evdev-based crates (`hotkey_listener` pattern) need `input` group
  membership — heavier install friction.
- `gilrs` = evdev, works on X11 and Wayland. ✅
- Verdict: full support on X11; Wayland needs an evdev fallback for
  clicks/keys. ⚠️

### macOS
- `rdev` needs **Accessibility permission** (System Settings → Privacy).
  Without it, callbacks are silently ignored — the app must detect and
  prompt (crates like `handy-keys` expose `check_accessibility()`).
- `gilrs` works (no force feedback).
- Verdict: works after one-time permission grant. ⚠️ (prompt UX required)

## Tauri v2 integration pattern

```rust
// src-tauri/src/main.rs
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            // Input hooks run on a plain OS thread next to Tauri.
            // rdev::listen blocks, so it gets its own thread;
            // cursor/gamepad pollers are already threaded.
            std::thread::spawn(move || {
                flickcut_input::run(move |ev| {
                    // ev: serde_json::Value {t_ms, source, action, ...}
                    handle.emit("flickcut://input-event", ev).ok();
                });
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri failed");
}
```

Frontend (`@tauri-apps/api/event`):

```ts
import { listen } from '@tauri-apps/api/event';
await listen('flickcut://input-event', (e) => {
  // e.payload.t_ms lines up with the video timeline
});
```

Notes:
- Keep hook threads as OS threads (`std::thread`), not Tokio tasks —
  `rdev::listen` blocks and some platform APIs dislike being moved.
- `t_ms` uses one shared `Instant` epoch created at Record press,
  exactly like the Python `RecordingSession` contract.
- For a long-running recorder, prefer batching cursor points
  (e.g. emit every 10 points) over one event per poll to avoid
  IPC overhead at 30 Hz.

## Prototype

`spike/input-hooks/` — standalone binary (no Tauri dependency, to keep
the spike light). Prints NDJSON events to stdout:

```json
{"t_ms":1234.5,"source":"cursor","action":"move","x":960,"y":540}
{"t_ms":1250.1,"source":"os","action":"click_left"}
{"t_ms":1300.7,"source":"gamepad","action":"btn_down","button":"RightTrigger","fire":true}
```

```sh
cd spike/input-hooks
cargo run --release
# click / move mouse / press gamepad buttons; Ctrl-C to stop
```

## Recommendation

1. `rdev` + `device_query` + `gilrs` cover all three platforms for v1.
2. Linux/Wayland click events are the only real gap — decide whether
   to require X11 for v1 or budget an evdev fallback.
3. macOS needs an Accessibility-permission prompt on first run.
4. Next step: wrap the spike as a `flickcut-input` lib crate with the
   Tauri `setup()` wiring above, then port `session.py`'s clock math.
