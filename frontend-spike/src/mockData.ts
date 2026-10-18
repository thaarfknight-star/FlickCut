/**
 * Mock helpers for the editor — pure functions only.
 * Types live in ./ipc/types; components must NOT import mock data,
 * they go through ./ipc/tauriApi.
 */
import type { EffectType, Marker, Region } from "./ipc/types";

export type { EffectType, Marker, Region };

export const EFFECT_LABELS: Record<EffectType, string> = {
  slowmo: "Slow-mo",
  curve: "Speed curve",
  zoomin: "Zoom-in",
  zoomout: "Zoom-out",
};

export const DURATION = 124.5; // mock video length, seconds

export function describeRegion(r: Region): string {
  const label = EFFECT_LABELS[r.effect];
  return `${label} ${r.start.toFixed(1)}s–${r.end.toFixed(1)}s (${r.intensity}%)`;
}

export function describeMarker(m: Marker): string {
  const label =
    m.type === "kill" ? "Kill" : m.type === "hit" ? "Hit" : "Exciting";
  return `${label} @ ${m.timestamp.toFixed(1)}s`;
}

export function formatTime(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/**
 * Suggest edit regions from markers — mirrors the legacy
 * `regions_from_moments` (slow-mo + zoom-in around each kill).
 */
export function regionsFromMarkers(
  markers: Marker[],
  duration: number
): Region[] {
  const out: Region[] = [];
  for (const m of markers) {
    if (m.type !== "kill") continue;
    out.push({
      start: Math.max(0, m.timestamp - 0.4),
      end: Math.min(duration, m.timestamp + 0.8),
      effect: "slowmo",
      intensity: Math.round(m.intensity),
    });
    out.push({
      start: Math.max(0, m.timestamp - 0.3),
      end: Math.min(duration, m.timestamp + 1.0),
      effect: "zoomin",
      intensity: 65,
    });
  }
  return out.sort((a, b) => a.start - b.start);
}
