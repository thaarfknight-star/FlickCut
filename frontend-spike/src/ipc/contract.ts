/**
 * FlickCut IPC contract v1 — frozen, see docs/ipc-contract-v1.md.
 * Do not change without project-lead approval. This file is a typed
 * index of the contract; the normative text lives in the repo doc.
 *
 * 6 commands (frontend -> Rust via `invoke`):
 *   list_sources() -> { screens, windows }
 *   start_recording(source_id) -> { session_id }
 *   stop_recording() -> { video_path, duration_sec }
 *   analyze_moments(session_id) -> { markers }
 *   render_video(video_path, regions) -> { output_path }
 *   import_video(path) -> { session_id }
 *
 * 3 events (Rust -> frontend via `listen`):
 *   flickcut://recording-tick  { elapsed_sec }
 *   flickcut://render-progress  { percent }
 *   flickcut://input-event      { t_ms, source, action, detail }
 *
 * NOTE: `input_*` commands (input_start/stop/trail/clear) are internal
 * to the flickcut-input crate — the frontend never calls them.
 */

export const COMMANDS = {
  LIST_SOURCES: "list_sources",
  START_RECORDING: "start_recording",
  STOP_RECORDING: "stop_recording",
  ANALYZE_MOMENTS: "analyze_moments",
  RENDER_VIDEO: "render_video",
  IMPORT_VIDEO: "import_video",
} as const;

export const EVENTS = {
  RECORDING_TICK: "flickcut://recording-tick",
  RENDER_PROGRESS: "flickcut://render-progress",
  INPUT_EVENT: "flickcut://input-event",
} as const;
