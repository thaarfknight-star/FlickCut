import type { EffectType } from "./ipc/types";

export const EFFECT_LABELS: Record<EffectType, string> = {
  slowmo: "Slow-mo",
  curve: "Speed curve",
  zoomin: "Zoom-in",
  zoomout: "Zoom-out",
};
