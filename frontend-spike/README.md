# FlickCut — React Frontend (Recordly-style)

Runnable React prototype of the FlickCut Tauri v2 frontend.
**Mock IPC only** — no Tauri shell, no real capture.

## Run it

```bash
cd frontend-spike
npm install
npm run dev      # → http://localhost:5173
```

## Flow (Recordly-style)

1. **Launch → floating bar.** A pill floats bottom-center (in Tauri: a
   second window — `decorations: false`, `alwaysOnTop: true`; see
   `WINDOWS_SPEC.md`). Source picker, mic toggle, big Record button.
2. **Record** → bar shows pulsing dot, timer, pause/resume, stop, and an
   "open editor" button.
3. **Stop → the editor opens automatically.** Markers are analyzed, edit
   regions are suggested on the timeline.

## What is in the editor

- **Slim header** — logo, title, timer, status pill, minimize-to-bar.
- **Library dock** — markers list (delete/clear) + edit regions list.
- **Center preview** — mock frame (real video via Tauri later).
- **Settings dock** — region effect, intensity slider with live %,
  start/end, add/delete; import, auto-edit, export; progress + log.
- **Timeline** — full width, region bars, marker diamonds, playhead.

## IPC

The normative contract is `docs/ipc-contract-v1.md` on main (frozen).
`src/ipc/tauriApi.ts` is the only module components import — real
`invoke`/`listen` inside Tauri, mock backend in the browser.

Note: pause/resume are contract **v1.1** (`pause_recording` /
`resume_recording`) — the shell implements them, the mock mirrors the shapes.
