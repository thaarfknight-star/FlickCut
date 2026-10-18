# FlickCut IPC Contract v1

Frozen by the project lead. Do not change without approval.
All names, arguments, and payload shapes below are normative.

## Commands (frontend -> Rust)

### `list_sources()`
Lists available capture targets.
Returns: `{ screens: [{ id: string, name: string }], windows: [{ id: string, title: string }] }`

### `start_recording(source_id: string)`
Starts capture and input telemetry on one shared session clock.
Returns: `{ session_id: string }`
Emits `flickcut://recording-tick` every second while recording.

### `stop_recording()`
Stops the active session.
Returns: `{ video_path: string, duration_sec: number }`

### `analyze_moments(session_id: string)`
Runs moment detection over the session telemetry.
Returns: `{ markers: Marker[] }`

### `render_video(video_path: string, regions: Region[])`
Renders the final MP4 with the regions applied.
Returns: `{ output_path: string }`
Emits `flickcut://render-progress` while rendering.

### `import_video(path: string)`
Imports an existing video file as a session (no telemetry).
Returns: `{ session_id: string }`

## Events (Rust -> frontend)

### `flickcut://recording-tick`
Payload: `{ elapsed_sec: number }`

### `flickcut://render-progress`
Payload: `{ percent: number }` (0-100)

### `flickcut://input-event`
Discrete input events (clicks, keys, gamepad buttons).
Payload: `{ t_ms: number, source: string, action: string, detail: object }`
Note: the 30 Hz cursor trail is polled via `input_trail`, never emitted.

## Types

```ts
interface Marker {
  timestamp: number;  // seconds
  type: "kill" | "hit" | "exciting";
  intensity: number;  // 0-100
  duration: number;   // seconds
  x: number;         // normalized 0-1 (focus point)
  y: number;         // normalized 0-1 (focus point)
}

interface Region {
  start: number;     // seconds
  end: number;       // seconds
  effect: "slowmo" | "curve" | "zoomin" | "zoomout";
  intensity: number; // 0-100
}
```

## Internal module commands (not part of the frontend contract)

Registered for testing and debugging; the frontend uses the six
commands above. The app-level `start_recording` starts these internally.
- `input_start()`, `input_stop()`, `input_trail()`, `input_clear()`
  (flickcut-input crate)
