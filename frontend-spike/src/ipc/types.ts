/**
 * Shared IPC types — mirror docs/ipc-contract-v1.md 1:1.
 * These are the shapes that cross the Tauri invoke/listen boundary.
 */

export type MomentType = "kill" | "hit" | "exciting";
export type EffectType = "slowmo" | "curve" | "zoomin" | "zoomout";

export interface ScreenSource {
  id: string;
  name: string;
}

export interface WindowSource {
  id: string;
  title: string;
}

export interface SourceList {
  screens: ScreenSource[];
  windows: WindowSource[];
}

export interface Marker {
  timestamp: number; // seconds
  type: MomentType;
  intensity: number; // 0-100
  duration: number; // seconds
  x: number; // normalized 0-1 (action focus point)
  y: number; // normalized 0-1 (action focus point)
}

export interface Region {
  start: number; // seconds
  end: number; // seconds
  effect: EffectType;
  intensity: number; // 0-100
}

export interface RecordingTick {
  elapsed_sec: number;
}

export interface RenderProgress {
  percent: number; // 0-100
}

export interface InputEvent {
  t_ms: number;
  source: string;
  action: string;
  detail: Record<string, unknown>;
}
