/**
 * FlickCut IPC contract (proposed v1).
 *
 * 6 commands (frontend -> Rust via `invoke`) and 2 events (Rust ->
 * frontend via `listen`). The `tauriApi` module is the ONLY place that
 * touches this contract — components never call `invoke`/`listen`
 * directly, so swapping the mock for the real shell changes one file.
 *
 * Naming follows Tauri convention: snake_case commands, kebab-case events.
 *
 * Commands:
 *   list_sources() -> CaptureSource[]
 *   start_recording(source_id: string) -> { session_id: string }
 *   stop_recording() -> RecordingSummary
 *   detect_moments(video_path: string) -> Moment[]
 *   render_preview(video_path: string, regions: Region[]) -> { preview_path: string }
 *   export_highlights(video_path: string, regions: Region[], output_path: string)
 *       -> { output_path: string }
 *
 * Events:
 *   moment-detected -> Moment            (live, while recording)
 *   render-progress -> RenderProgress    (during render_preview / export_highlights)
 */

export const COMMANDS = {
  LIST_SOURCES: "list_sources",
  START_RECORDING: "start_recording",
  STOP_RECORDING: "stop_recording",
  DETECT_MOMENTS: "detect_moments",
  RENDER_PREVIEW: "render_preview",
  EXPORT_HIGHLIGHTS: "export_highlights",
} as const;

export const EVENTS = {
  MOMENT_DETECTED: "moment-detected",
  RENDER_PROGRESS: "render-progress",
} as const;
