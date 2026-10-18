# IPC Contract

The normative contract is **`docs/ipc-contract-v1.md`** on main (frozen by
the project lead — do not change without approval).

This directory implements it:

- `src/ipc/contract.ts` — typed index of the 6 commands + 3 events
- `src/ipc/types.ts` — shared IPC shapes (Marker, Region, …)
- `src/ipc/tauriApi.ts` — the ONLY module components import; delegates to
  real `invoke`/`listen` inside Tauri, mock backend in a plain browser
- `src/ipc/mockBackend.ts` — mock implementation of the contract

## Quick reference

Commands: `list_sources` → `{ screens, windows }` ·
`start_recording(source_id)` → `{ session_id }` ·
`stop_recording()` → `{ video_path, duration_sec }` ·
`analyze_moments(session_id)` → `{ markers }` ·
`render_video(video_path, regions)` → `{ output_path }` ·
`import_video(path)` → `{ session_id }`

Events: `flickcut://recording-tick` `{ elapsed_sec }` ·
`flickcut://render-progress` `{ percent }` ·
`flickcut://input-event` `{ t_ms, source, action, detail }`
