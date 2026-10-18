# FlickCut IPC Contract v1 (proposed)

The interface between the React frontend and the Tauri v2 Rust shell.
The frontend talks to it **only** through `src/ipc/tauriApi.ts` — components
never call `invoke`/`listen` directly.

## Commands (frontend → Rust, via `invoke`)

| # | Command | Args | Returns |
|---|---------|------|---------|
| 1 | `list_sources` | — | `CaptureSource[]` |
| 2 | `start_recording` | `{ source_id: string }` | `{ session_id: string }` |
| 3 | `stop_recording` | — | `RecordingSummary` |
| 4 | `detect_moments` | `{ video_path: string }` | `Moment[]` |
| 5 | `render_preview` | `{ video_path: string, regions: Region[] }` | `{ preview_path: string }` |
| 6 | `export_highlights` | `{ video_path: string, regions: Region[], output_path: string }` | `{ output_path: string }` |

## Events (Rust → frontend, via `listen`)

| # | Event | Payload | When |
|---|-------|---------|------|
| 1 | `moment-detected` | `Moment` | live, while recording |
| 2 | `render-progress` | `RenderProgress { job_id, progress: 0..100 }` | during `render_preview` / `export_highlights` |

## Types

```ts
interface CaptureSource { id: string; name: string; kind: "screen" | "window" }
interface Moment {
  id: string; timestamp: number; type: "kill" | "hit" | "exciting";
  intensity: number; x?: number; y?: number;  // x/y: normalized action focus
}
interface Region {
  id: string; start: number; end: number;
  effect: "slowmo" | "curve" | "zoomin" | "zoomout"; intensity: number;
}
interface RecordingSummary {
  session_id: string; video_path: string;
  duration_s: number; moments: Moment[];
}
interface RenderProgress { job_id: string; progress: number }
```

## Mock mode

Outside Tauri (`window.__TAURI__` absent), `tauriApi` delegates to
`src/ipc/mockBackend.ts`, which implements the same contract with timers
and fake data — the editor runs standalone via `vite dev`.

## Status

Proposed by TAgent4 (2026-10-18). TAgent5's shell will implement the Rust
side; renames go through `src/ipc/contract.ts` only.
