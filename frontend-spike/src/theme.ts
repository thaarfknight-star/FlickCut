/**
 * Theme — single source of truth for the FlickCut editor palette.
 *
 * Derived from the FlickCut logo (chrome "F" on black). To apply a new
 * official palette, replace these values — nothing else needs to change.
 * Mirrors the THEME dict in the legacy PyQt main.py.
 */
export const THEME = {
  bg: "#0A0C10", // app background (logo black)
  surface: "#10141B", // panels / docks
  card: "#151B25", // cards
  input: "#0C1016", // list / text inputs
  border: "#494c55", // hairlines (darkest chrome)
  text: "#e3e6ec", // chrome highlight
  muted: "#a1a8b6", // steel gray
  faint: "#6d717b", // disabled text
  accent: "#c0c8d8", // light chrome (interactive highlights)
  accent2: "#8b8f98", // mid chrome
  danger: "#FF3B30",
  success: "#34D17B",
  amber: "#FFB020",
} as const;

/** Region bar colors by effect (functional, not theme). */
export const REGION_COLORS: Record<string, string> = {
  slowmo: "#2EA6FF",
  curve: "#2ED9A6",
  zoomin: "#FFB020",
  zoomout: "#FFB020",
};

/** Moment diamond colors by type (functional, not theme). */
export const MOMENT_COLORS: Record<string, string> = {
  kill: "#FF3B30",
  hit: "#FFB020",
  exciting: "#FF00D4",
};
