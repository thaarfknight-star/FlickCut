// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! FlickCut moment detection engine.
//!
//! Port of `moments.py`: analyzes input events and detects "highlight
//! moments" — rapid / sequential fire presses that likely indicate a kill
//! or a hit.
//!
//! Each [`Moment`] = timestamp + type (kill/hit/exciting) + intensity +
//! action-point (x, y). [`detect_moments`] returns moments sorted by
//! timestamp. [`Marker`] is the IPC contract schema
//! (`docs/ipc-contract-v1.md`): timestamp seconds, type, intensity 0-100,
//! duration seconds, x/y normalized 0-1.
//!
//! Works on [`flickcut_input::InputEvent`] (`detail` is string-valued):
//! fire = mouse `click_left` or gamepad event with `detail["fire"]=="true"`.
//! Click positions come from event detail `x`/`y` when present, otherwise
//! from the nearest cursor-trail point, otherwise the frame center.

use flickcut_input::{CursorPoint, InputEvent};
use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Tuning (mirrors moments.py constants)
// ---------------------------------------------------------------------------
/// Max gap (s) between presses to belong to the same burst.
pub const BURST_GAP: f64 = 0.6;
/// Kill: dense burst (>= KILL_MIN_PRESSES within KILL_WINDOW s) + pause.
pub const KILL_MIN_PRESSES: usize = 3;
pub const KILL_WINDOW: f64 = 0.9;
pub const KILL_PAUSE: f64 = 1.2;
/// Hit: small burst (>= HIT_MIN_PRESSES within HIT_WINDOW s).
pub const HIT_MIN_PRESSES: usize = 2;
pub const HIT_WINDOW: f64 = 0.5;
/// Exciting: sustained activity.
pub const EXCITING_MIN_PRESSES: usize = 8;
pub const EXCITING_MIN_SPAN: f64 = 2.5;
/// Reference presses-per-second for intensity normalization.
pub const REF_PPS: f64 = 8.0;

// ---------------------------------------------------------------------------
// Moment + marker
// ---------------------------------------------------------------------------

/// Moment type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MomentType {
    Kill,
    Hit,
    Exciting,
}

impl MomentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MomentType::Kill => "kill",
            MomentType::Hit => "hit",
            MomentType::Exciting => "exciting",
        }
    }
}

/// A detected highlight moment (timestamps in seconds, intensity 0-1).
#[derive(Debug, Clone)]
pub struct Moment {
    /// Start of the moment, seconds.
    pub timestamp: f64,
    pub kind: MomentType,
    /// 0.0 - 1.0 (clamped).
    pub intensity: f64,
    /// Seconds (clamped >= 0).
    pub duration: f64,
    /// Fire presses inside the moment.
    pub event_count: usize,
    /// Normalized action-point x, 0..1 (clamped).
    pub x: f64,
    /// Normalized action-point y, 0..1 (clamped).
    pub y: f64,
}

fn clamp01(v: f64) -> f64 {
    v.max(0.0).min(1.0)
}

impl Moment {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timestamp: f64,
        kind: MomentType,
        intensity: f64,
        duration: f64,
        event_count: usize,
        x: f64,
        y: f64,
    ) -> Self {
        Self {
            timestamp,
            kind,
            intensity: clamp01(intensity),
            duration: duration.max(0.0),
            event_count,
            x: clamp01(x),
            y: clamp01(y),
        }
    }

    /// Contract marker: intensity scaled to 0-100 per ipc-contract-v1.
    pub fn to_marker(&self) -> Marker {
        Marker {
            timestamp: round3(self.timestamp),
            kind: self.kind.as_str().to_string(),
            intensity: (round3(self.intensity) * 100.0).round(),
            duration: round3(self.duration),
            x: round3(self.x),
            y: round3(self.y),
        }
    }
}

/// IPC contract marker (`docs/ipc-contract-v1.md`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Marker {
    /// Seconds.
    pub timestamp: f64,
    /// "kill" | "hit" | "exciting".
    #[serde(rename = "type")]
    pub kind: String,
    /// 0-100.
    pub intensity: f64,
    /// Seconds.
    pub duration: f64,
    /// Normalized 0-1 focus point.
    pub x: f64,
    /// Normalized 0-1 focus point.
    pub y: f64,
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

fn is_fire(ev: &InputEvent) -> bool {
    if ev.source == "mouse" && ev.action == "click_left" {
        return true;
    }
    if ev.source == "gamepad" {
        return ev.detail.get("fire").map(|s| s == "true").unwrap_or(false);
    }
    false
}

fn detail_f64(ev: &InputEvent, key: &str) -> Option<f64> {
    ev.detail.get(key).and_then(|s| s.parse::<f64>().ok())
}

fn intensity(press_count: usize, span: f64) -> f64 {
    if span <= 0.0 || press_count == 0 {
        return 0.0;
    }
    clamp01((press_count as f64 / span) / REF_PPS)
}

/// Nearest cursor-trail position to `t_ms`, if the trail is non-empty.
fn trail_position(trail: &[CursorPoint], t_ms: f64) -> Option<(f64, f64)> {
    trail
        .iter()
        .min_by(|a, b| {
            (a.t_ms - t_ms)
                .abs()
                .partial_cmp(&(b.t_ms - t_ms).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|p| (p.x as f64, p.y as f64))
}

/// Centroid of click positions in the burst, normalized 0..1.
///
/// Position source, in order: event detail `x`/`y`, nearest cursor-trail
/// point, else (0.5, 0.5). Normalized by `screen_size` (width, height);
/// falls back to detail `screen_w`/`screen_h` when present.
fn burst_center(
    burst: &[(f64, &InputEvent)],
    trail: &[CursorPoint],
    screen_size: Option<(f64, f64)>,
) -> (f64, f64) {
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut sw: Option<f64> = None;
    let mut sh: Option<f64> = None;
    for (t_s, ev) in burst {
        match (detail_f64(ev, "x"), detail_f64(ev, "y")) {
            (Some(x), Some(y)) => {
                xs.push(x);
                ys.push(y);
            }
            _ => {
                // Fall back to the cursor trail for position-less clicks.
                if let Some((tx, ty)) = trail_position(trail, t_s * 1000.0) {
                    xs.push(tx);
                    ys.push(ty);
                }
            }
        }
        if sw.is_none() {
            match (detail_f64(ev, "screen_w"), detail_f64(ev, "screen_h")) {
                (Some(w), Some(h)) => {
                    sw = Some(w);
                    sh = Some(h);
                }
                _ => {}
            }
        }
    }
    if xs.is_empty() {
        return (0.5, 0.5);
    }
    let (w, h) = match screen_size {
        Some((w, h)) => (w, h),
        None => match (sw, sh) {
            (Some(w), Some(h)) => (w, h),
            _ => return (0.5, 0.5),
        },
    };
    if w <= 0.0 || h <= 0.0 {
        return (0.5, 0.5);
    }
    let cx = clamp01(xs.iter().sum::<f64>() / xs.len() as f64 / w);
    let cy = clamp01(ys.iter().sum::<f64>() / ys.len() as f64 / h);
    (cx, cy)
}

/// Detect highlight moments from input events, sorted by timestamp.
///
/// `trail`: cursor-trail points used to position clicks that carry no
/// `x`/`y` in their detail. `screen_size`: optional (width, height) used
/// to normalize positions into the moment's x/y action point.
pub fn detect_moments(
    events: &[InputEvent],
    trail: &[CursorPoint],
    screen_size: Option<(f64, f64)>,
) -> Vec<Moment> {
    // Sorted (timestamp_seconds, event) fire presses.
    let mut pairs: Vec<(f64, &InputEvent)> = events
        .iter()
        .filter(|e| is_fire(e))
        .map(|e| (e.t_ms / 1000.0, e))
        .collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if pairs.is_empty() {
        return Vec::new();
    }

    // Group into bursts; a gap > BURST_GAP starts a new burst.
    let mut bursts: Vec<Vec<(f64, &InputEvent)>> = Vec::new();
    let mut cur: Vec<(f64, &InputEvent)> = Vec::new();
    for p in pairs {
        if let Some(last) = cur.last() {
            if p.0 - last.0 > BURST_GAP {
                bursts.push(std::mem::take(&mut cur));
            }
        }
        cur.push(p);
    }
    if !cur.is_empty() {
        bursts.push(cur);
    }

    let mut moments: Vec<Moment> = Vec::new();
    for (i, b) in bursts.iter().enumerate() {
        let times: Vec<f64> = b.iter().map(|(t, _)| *t).collect();
        let span = if times.len() > 1 {
            times[times.len() - 1] - times[0]
        } else {
            0.0
        };
        let n = b.len();
        let pause = if i + 1 < bursts.len() {
            bursts[i + 1][0].0 - times[times.len() - 1]
        } else {
            f64::INFINITY
        };

        let kind = if n >= KILL_MIN_PRESSES && span <= KILL_WINDOW && pause >= KILL_PAUSE {
            Some(MomentType::Kill)
        } else if n >= HIT_MIN_PRESSES && span <= HIT_WINDOW {
            Some(MomentType::Hit)
        } else if n >= EXCITING_MIN_PRESSES && span >= EXCITING_MIN_SPAN {
            Some(MomentType::Exciting)
        } else {
            None
        };

        if let Some(kind) = kind {
            let (cx, cy) = burst_center(b, trail, screen_size);
            moments.push(Moment::new(
                times[0],
                kind,
                intensity(n, span.max(0.2)),
                span,
                n,
                cx,
                cy,
            ));
        }
    }
    moments
}

/// Convert moments to IPC contract markers.
pub fn moments_to_markers(moments: &[Moment]) -> Vec<Marker> {
    moments.iter().map(|m| m.to_marker()).collect()
}

// ---------------------------------------------------------------------------
// Session API (implements the ipc-contract-v1 analyze_moments command)
// ---------------------------------------------------------------------------

/// Telemetry captured for one recording session.
#[derive(Debug, Clone, Default)]
pub struct SessionData {
    pub session_id: String,
    pub events: Vec<InputEvent>,
    pub trail: Vec<CursorPoint>,
    /// (width, height) of the captured screen, when known.
    pub screen_size: Option<(f64, f64)>,
}

/// Session registry queried by `analyze_moments(session_id)`.
pub type SessionStore = HashMap<String, SessionData>;

/// Run moment detection over a session's telemetry -> contract markers.
///
/// Returns an empty vec when the session id is unknown.
pub fn analyze_moments(session_id: &str, sessions: &SessionStore) -> Vec<Marker> {
    match sessions.get(session_id) {
        Some(s) => moments_to_markers(&detect_moments(&s.events, &s.trail, s.screen_size)),
        None => Vec::new(),
    }
}

/// One-call pipeline: events + trail -> markers (no session registry).
pub fn analyze(
    events: &[InputEvent],
    trail: &[CursorPoint],
    screen_size: Option<(f64, f64)>,
) -> Vec<Marker> {
    moments_to_markers(&detect_moments(events, trail, screen_size))
}

// ---------------------------------------------------------------------------
// Unit tests — parity with moments.py behavior + contract compliance
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn detail(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn clicks(start_ms: f64, n: usize, gap_ms: f64) -> Vec<InputEvent> {
        (0..n)
            .map(|i| InputEvent {
                t_ms: start_ms + i as f64 * gap_ms,
                source: "mouse",
                action: "click_left".to_string(),
                detail: HashMap::new(),
            })
            .collect()
    }

    fn clicks_at(start_ms: f64, n: usize, gap_ms: f64, x: f64, y: f64) -> Vec<InputEvent> {
        (0..n)
            .map(|i| InputEvent {
                t_ms: start_ms + i as f64 * gap_ms,
                source: "mouse",
                action: "click_left".to_string(),
                detail: detail(&[
                    ("x", &(x + i as f64 * 10.0).to_string()),
                    ("y", &(y + i as f64 * 5.0).to_string()),
                ]),
            })
            .collect()
    }

    fn pad_clicks(start_ms: f64, n: usize, gap_ms: f64, fire: bool) -> Vec<InputEvent> {
        (0..n)
            .map(|i| InputEvent {
                t_ms: start_ms + i as f64 * gap_ms,
                source: "gamepad",
                action: "btn_down".to_string(),
                detail: detail(&[("fire", if fire { "true" } else { "false" })]),
            })
            .collect()
    }

    // -- parity with moments.py -------------------------------------------

    #[test]
    fn kill_dense_burst_then_pause() {
        let m = detect_moments(&clicks(10_000.0, 4, 150.0), &[], None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Kill);
        assert!((m[0].timestamp - 10.0).abs() < 1e-9);
        assert!((m[0].duration - 0.45).abs() < 1e-9);
        assert_eq!(m[0].event_count, 4);
    }

    #[test]
    fn hit_small_burst() {
        let m = detect_moments(&clicks(20_000.0, 2, 200.0), &[], None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Hit);
    }

    #[test]
    fn exciting_sustained() {
        let m = detect_moments(&clicks(30_000.0, 10, 300.0), &[], None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Exciting);
    }

    #[test]
    fn lone_click_ignored() {
        assert!(detect_moments(&clicks(50_000.0, 1, 0.0), &[], None).is_empty());
    }

    #[test]
    fn right_click_not_fire() {
        let evs: Vec<InputEvent> = (0..5)
            .map(|i| InputEvent {
                t_ms: 60_000.0 + i as f64 * 100.0,
                source: "mouse",
                action: "click_right".to_string(),
                detail: HashMap::new(),
            })
            .collect();
        assert!(detect_moments(&evs, &[], None).is_empty());
    }

    #[test]
    fn gamepad_fire_counts() {
        let m = detect_moments(&pad_clicks(70_000.0, 4, 150.0, true), &[], None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Kill);
    }

    #[test]
    fn gamepad_non_fire_ignored() {
        assert!(detect_moments(&pad_clicks(80_000.0, 5, 100.0, false), &[], None).is_empty());
    }

    #[test]
    fn empty_input() {
        assert!(detect_moments(&[], &[], None).is_empty());
    }

    #[test]
    fn output_sorted() {
        let mut evs = clicks(30_000.0, 10, 300.0);
        evs.extend(clicks(10_000.0, 4, 150.0));
        let m = detect_moments(&evs, &[], None);
        let ts: Vec<f64> = m.iter().map(|x| x.timestamp).collect();
        let mut sorted = ts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(ts, sorted);
    }

    // -- position focus ----------------------------------------------------

    #[test]
    fn position_centroid_normalized() {
        let m = detect_moments(
            &clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0),
            &[],
            Some((1920.0, 1080.0)),
        );
        assert_eq!(m.len(), 1);
        assert!((m[0].x - 0.758).abs() < 0.01, "x={}", m[0].x);
        assert!((m[0].y - 0.757).abs() < 0.01, "y={}", m[0].y);
    }

    #[test]
    fn position_from_trail_when_no_detail() {
        // clicks carry no x/y; trail has the cursor at (1440, 810)
        let trail: Vec<CursorPoint> = (0..40)
            .map(|i| CursorPoint {
                t_ms: 9_000.0 + i as f64 * 100.0,
                x: 1440,
                y: 810,
            })
            .collect();
        let m = detect_moments(&clicks(10_000.0, 4, 150.0), &trail, Some((1920.0, 1080.0)));
        assert_eq!(m.len(), 1);
        assert!((m[0].x - 0.75).abs() < 0.01, "x={}", m[0].x);
        assert!((m[0].y - 0.75).abs() < 0.01, "y={}", m[0].y);
    }

    #[test]
    fn position_defaults_to_center() {
        let m = detect_moments(&clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0), &[], None);
        assert_eq!((m[0].x, m[0].y), (0.5, 0.5));
    }

    // -- contract compliance ------------------------------------------------

    #[test]
    fn marker_intensity_is_0_to_100() {
        // Python parity intensity 1.0 -> contract 100
        let m = detect_moments(&clicks(10_000.0, 4, 150.0), &[], None);
        let mk = m[0].to_marker();
        assert_eq!(mk.intensity, 100.0);
        // Python parity intensity 0.463 -> contract 46
        let m2 = detect_moments(&clicks(30_000.0, 10, 300.0), &[], None);
        assert_eq!(m2[0].to_marker().intensity, 46.0);
    }

    #[test]
    fn marker_serializes_to_contract_json() {
        let mk = analyze(&clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0), &[], Some((1920.0, 1080.0)));
        let json = serde_json::to_string(&mk[0]).unwrap();
        assert!(json.contains("\"type\":\"kill\""), "{}", json);
        assert!(json.contains("\"timestamp\":10.0"), "{}", json);
        assert!(json.contains("\"intensity\":100.0"), "{}", json);
        assert!(json.contains("\"x\":0.758"), "{}", json);
    }

    #[test]
    fn analyze_moments_session_lookup() {
        let mut store: SessionStore = HashMap::new();
        store.insert(
            "sess-1".to_string(),
            SessionData {
                session_id: "sess-1".to_string(),
                events: clicks(10_000.0, 4, 150.0),
                trail: vec![],
                screen_size: None,
            },
        );
        let out = analyze_moments("sess-1", &store);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "kill");
        // unknown session -> empty, no panic
        assert!(analyze_moments("nope", &store).is_empty());
    }

    #[test]
    fn parity_marker_values_match_python() {
        // Same scenario verified bit-for-bit against moments.py:
        // kill@10s x=0.758 y=0.757, hit@20s, exciting@30s intensity 46
        let mut evs = clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0);
        evs.extend(clicks_at(20_000.0, 2, 200.0, 100.0, 100.0));
        evs.extend(clicks(30_000.0, 10, 300.0));
        evs.extend(pad_clicks(70_000.0, 4, 150.0, true));
        let mk = analyze(&evs, &[], Some((1920.0, 1080.0)));
        assert_eq!(mk.len(), 4);
        assert_eq!(mk[0].kind, "kill");
        assert!((mk[0].timestamp - 10.0).abs() < 1e-9);
        assert!((mk[0].x - 0.758).abs() < 0.01);
        assert!((mk[0].y - 0.757).abs() < 0.01);
        assert_eq!(mk[1].kind, "hit");
        assert!((mk[1].x - 0.055).abs() < 0.01);
        assert_eq!(mk[2].kind, "exciting");
        assert_eq!(mk[2].intensity, 46.0); // python 0.463 * 100
        assert_eq!(mk[3].kind, "kill"); // gamepad
        assert_eq!((mk[3].x, mk[3].y), (0.5, 0.5));
    }
}
