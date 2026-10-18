/**
 * Shared IPC types — mirror the Rust side (src-tauri) 1:1.
 * These are the shapes that cross the Tauri invoke/listen boundary.
 */

export type MomentType = "kill" | "hit" | "exciting";
export type EffectType = "slowmo" | "curve" | "zoomin" | "zoomout";

export interface CaptureSource {
  id: string;
  name: string;
  kind: "screen" | "window";
}

export interface Moment {
  id: string;
  timestamp: number; // seconds, video-time
  type: MomentType;
  intensity: number; // 0..100
  /** Normalized action focus point (0..1) — zoom follows this. */
  x?: number;
  y?: number;
}

export interface Region {
  id: string;
  start: number; // seconds
  end: number; // seconds
  effect: EffectType;
  intensity: number; // 0..100
}

export interface RecordingSummary {
  session_id: string;
  video_path: string;
  duration_s: number;
  moments: Moment[];
}

export interface RenderProgress {
  job_id: string;
  progress: number; // 0..100
}
