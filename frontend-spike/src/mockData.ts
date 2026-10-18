/** Mock data for the editor spike — mirrors the legacy session pipeline. */

export type MomentType = "kill" | "hit" | "exciting";
export type EffectType = "slowmo" | "curve" | "zoomin" | "zoomout";

export interface Moment {
  id: string;
  timestamp: number; // seconds
  type: MomentType;
  intensity: number; // 0..100
}

export interface Region {
  id: string;
  start: number; // seconds
  end: number; // seconds
  effect: EffectType;
  intensity: number; // 0..100
}

export const EFFECT_LABELS: Record<EffectType, string> = {
  slowmo: "Slow-mo",
  curve: "Speed curve",
  zoomin: "Zoom-in",
  zoomout: "Zoom-out",
};

export const DURATION = 124.5; // mock video length, seconds

export const MOCK_MOMENTS: Moment[] = [
  { id: "m1", timestamp: 12.4, type: "kill", intensity: 95 },
  { id: "m2", timestamp: 34.8, type: "hit", intensity: 60 },
  { id: "m3", timestamp: 47.2, type: "kill", intensity: 88 },
  { id: "m4", timestamp: 71.9, type: "exciting", intensity: 74 },
  { id: "m5", timestamp: 102.3, type: "kill", intensity: 91 },
];

export const MOCK_REGIONS: Region[] = [
  { id: "r1", start: 12.0, end: 13.2, effect: "slowmo", intensity: 79 },
  { id: "r2", start: 12.1, end: 13.4, effect: "zoomin", intensity: 62 },
  { id: "r3", start: 46.8, end: 48.0, effect: "slowmo", intensity: 85 },
  { id: "r4", start: 71.5, end: 72.8, effect: "curve", intensity: 55 },
  { id: "r5", start: 101.9, end: 103.1, effect: "zoomin", intensity: 70 },
];

export function describeRegion(r: Region): string {
  const label = EFFECT_LABELS[r.effect];
  return `${label} ${r.start.toFixed(1)}s–${r.end.toFixed(1)}s (${r.intensity}%)`;
}

export function formatTime(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
