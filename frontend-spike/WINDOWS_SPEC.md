# Control-bar window spec (for TAgent5's shell)

FlickCut launches into a **floating control bar** (Recordly-style), not the
big editor window. This is a second Tauri window.

## `tauri.conf.json` — windows

```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "FlickCut",
        "width": 1280,
        "height": 800,
        "minWidth": 960,
        "minHeight": 600,
        "visible": false
      },
      {
        "label": "control-bar",
        "title": "FlickCut",
        "width": 430,
        "height": 72,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "maximizable": false,
        "minimizable": false,
        "focus": false
      }
    ]
  }
}
```

## Positioning (runtime, Rust)

Bottom-center of the primary monitor. Do it at startup in `setup()` —
screen size is only known at runtime:

```rust
use tauri::{Manager, Position, PhysicalPosition};

let bar = app.get_webview_window("control-bar").unwrap();
let monitor = app.primary_monitor().unwrap().unwrap();
let size = monitor.size();
let scale = monitor.scale_factor();
let bar_w = (430.0 * scale) as i32;
let bar_h = (72.0 * scale) as i32;
let x = ((size.width as i32) - bar_w) / 2;
let y = (size.height as i32) - bar_h - (24.0 * scale) as i32;
bar.set_position(Position::Physical(PhysicalPosition { x, y })).unwrap();
bar.show().unwrap();
```

## Window choreography

| Event | Action |
|---|---|
| App launch | `control-bar` visible (the pill). `main` hidden. |
| `stop_recording` completes | show `main` (editor), keep bar visible (it stays always-on-top) |
| Bar "open editor" (⤢) clicked | show `main` |
| Editor "minimize to bar" clicked | hide `main` (bar keeps running) |
| App quit | close both |

The frontend tells the shell which view it is in via the existing
commands — no new IPC needed for the choreography except show/hide,
which the shell drives itself around `stop_recording`.

## Frontend side

`src/components/FloatingBar.tsx` is the bar UI. In the browser mock it
renders as a fixed pill bottom-center over a fake desktop; in Tauri it
is the entire content of the `control-bar` window (transparent body,
pill only).
