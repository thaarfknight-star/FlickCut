/**
 * Mock helpers for the editor — pure functions only.
 * Types live in ./ipc/types; components must NOT import mock data,
 * they go through ./ipc/tauriApi.
 */
import type { EffectType, Moment, Region } from "./ipc/types";

export { EFFECT_LABELS } from "./mockLabels";
export type { EffectType, Moment, Region };

export const DURATION = 124.5; // mock video length, seconds

export function describeRegion(r: Region): string {
  const label =
    { slowmo: "Slow-mo", curve: "Speed curve", zoomin: "Zoom-in", zoomout: "Zoom-out" }[
      r.effect
    ];
  return `${label} ${r.start.toFixed(1)}s–${r.end.toFixed(1)}s (${r.intensity}%)`;
}

export function formatTime(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/**
 * Suggest edit regions from moments — mirrors the legacy
 * `regions_from_moments` (slow-mo + zoom-in around each kill).
 */
export function regionsFromMoments(
  moments: Moment[],
  duration: number
): Region[] {
  const out: Region[] = [];
  let n = 0;
  for (const m of moments) {
    if (m.type !== "kill") continue;
    const s1 = Math.max(0, m.timestamp - 0.4);
    const e1 = Math.min(duration, m.timestamp + 0.8);
    out.push({
      id: `auto-s${++n}`,
      start: s1,
      end: e1,
      effect: "slowmo",
      intensity: Math.round(m.intensity),
    });
    const s2 = Math.max(0, m.timestamp - 0.3);
    const e2 = Math.min(duration, m.timestamp + 1.0);
    out.push({
      id: `auto-z${++n}`,
      start: s2,
      end: e2,
      effect: "zoomin",
      intensity: 65,
    });
  }
  return out.sort((a, b) => a.start - b.start);
}
