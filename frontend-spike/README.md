# FlickCut Editor — React Spike

Runnable React prototype of the Recordly-style editor for the FlickCut
Tauri v2 rewrite. **Mock data only** — no Tauri shell, no real capture.

## Run it

```bash
cd frontend-spike
npm install
npm run dev      # → http://localhost:5173
```

## What it shows

- **Header bar** — transparent logo, title + version, elapsed timer,
  READY/REC pill, Check-for-Updates + About buttons.
- **Library dock** — Moments list (delete/clear) + Edit Regions list.
- **Center preview** — mock gameplay frame (real video via Tauri later).
- **Settings dock** — region effect combo, intensity slider with live %,
  start/end inputs, Add/Delete region; Record/Stop, Import video,
  Auto-Edit + Preview, Export highlights; progress bar + log.
- **Timeline** — full width, region bars colored by effect, moment
  diamonds, playhead; click a bar to select the region.

## Theme

The whole palette lives in `frontend-spike/src/theme.ts` (mirrors the
legacy PyQt `THEME` dict): logo chrome `#e3e6ec / #c0c8d8 / #a1a8b6 /
#8b8f98 / #6d717b / #494c55` on `#0A0C10`.

## Status

Spike only — not wired to any backend. The Tauri v2 shell, capture
pipeline, and editor engine land separately.
