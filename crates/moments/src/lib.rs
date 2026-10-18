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
//! timestamp; [`Marker`] is the editor-timeline schema
//! (timestamp, type, intensity, duration, x, y).
//!
//! Input event model (mirrors `inputs.py` `InputEvent`):
//! - `t_ms`: milliseconds since recording start
//! - `source`: "mouse" | "gamepad"
//! - `action`: "click_left", "btn_down", "trigger_down", ...
//! - `detail`: fire buttons carry `fire = true`; mouse clicks carry
//!   `x`/`y` (and optionally `screen_w`/`screen_h`).

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
// Event model
// ---------------------------------------------------------------------------

/// One input event on the recording timeline.
#[derive(Debug, Clone)]
pub struct InputEvent {
    /// Milliseconds since recording start.
    pub t_ms: f64,
    /// "mouse" | "gamepad".
    pub source: String,
    /// "click_left", "btn_down", "trigger_down", ...
    pub action: String,
    /// Extra info: mouse clicks carry x/y; gamepad fire buttons carry fire.
    pub detail: HashMap<String, DetailValue>,
}

/// Detail values carried by an input event.
#[derive(Debug, Clone)]
pub enum DetailValue {
    Float(f64),
    Bool(bool),
    Str(String),
}

impl DetailValue {
    fn as_f64(&self) -> Option<f64> {
        match self {
            DetailValue::Float(v) => Some(*v),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            DetailValue::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

impl InputEvent {
    pub fn new(t_ms: f64, source: &str, action: &str) -> Self {
        Self {
            t_ms,
            source: source.to_string(),
            action: action.to_string(),
            detail: HashMap::new(),
        }
    }

    pub fn with_detail(mut self, key: &str, value: DetailValue) -> Self {
        self.detail.insert(key.to_string(), value);
        self
    }

    /// True for fire presses: mouse left click or gamepad fire button.
    pub fn is_fire(&self) -> bool {
        if self.source == "mouse" && self.action == "click_left" {
            return true;
        }
        if self.source == "gamepad" {
            return self
                .detail
                .get("fire")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        }
        false
    }

    fn detail_f64(&self, key: &str) -> Option<f64> {
        self.detail.get(key).and_then(|v| v.as_f64())
    }
}

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

/// A detected highlight moment (timestamps in seconds).
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

    /// Timeline marker for the editor:
    /// (timestamp, type, intensity, duration, x, y).
    pub fn to_marker(&self) -> Marker {
        Marker {
            timestamp: round3(self.timestamp),
            kind: self.kind.as_str().to_string(),
            intensity: round3(self.intensity),
            duration: round3(self.duration),
            x: round3(self.x),
            y: round3(self.y),
        }
    }
}

/// Editor-timeline marker (serializable schema).
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub timestamp: f64,
    pub kind: String,
    pub intensity: f64,
    pub duration: f64,
    pub x: f64,
    pub y: f64,
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

fn intensity(press_count: usize, span: f64) -> f64 {
    if span <= 0.0 || press_count == 0 {
        return 0.0;
    }
    clamp01((press_count as f64 / span) / REF_PPS)
}

/// Centroid of click positions in the burst, normalized 0..1.
///
/// Uses event detail x/y normalized by `screen_size` (width, height);
/// falls back to detail "screen_w"/"screen_h" when present, else (0.5, 0.5).
/// Gamepad presses carry no position and are skipped.
fn burst_center(burst: &[(f64, &InputEvent)], screen_size: Option<(f64, f64)>) -> (f64, f64) {
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut sw: Option<f64> = None;
    let mut sh: Option<f64> = None;
    for (_, ev) in burst {
        match (ev.detail_f64("x"), ev.detail_f64("y")) {
            (Some(x), Some(y)) => {
                xs.push(x);
                ys.push(y);
            }
            _ => {}
        }
        if sw.is_none() {
            match (ev.detail_f64("screen_w"), ev.detail_f64("screen_h")) {
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
/// `screen_size`: optional (width, height) used to normalize click
/// positions into the moment's x/y action point.
pub fn detect_moments(
    events: &[InputEvent],
    screen_size: Option<(f64, f64)>,
) -> Vec<Moment> {
    // Sorted (timestamp_seconds, event) fire presses.
    let mut pairs: Vec<(f64, &InputEvent)> = events
        .iter()
        .filter(|e| e.is_fire())
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
            let (cx, cy) = burst_center(b, screen_size);
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

/// Convert moments to editor-timeline markers.
pub fn moments_to_markers(moments: &[Moment]) -> Vec<Marker> {
    moments.iter().map(|m| m.to_marker()).collect()
}

/// One-call pipeline: input events -> timeline markers.
pub fn analyze(events: &[InputEvent], screen_size: Option<(f64, f64)>) -> Vec<Marker> {
    moments_to_markers(&detect_moments(events, screen_size))
}

// ---------------------------------------------------------------------------
// Unit tests — parity with moments.py behavior
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn clicks(start_ms: f64, n: usize, gap_ms: f64) -> Vec<InputEvent> {
        (0..n)
            .map(|i| InputEvent::new(start_ms + i as f64 * gap_ms, "mouse", "click_left"))
            .collect()
    }

    fn clicks_at(start_ms: f64, n: usize, gap_ms: f64, x: f64, y: f64) -> Vec<InputEvent> {
        (0..n)
            .map(|i| {
                InputEvent::new(start_ms + i as f64 * gap_ms, "mouse", "click_left")
                    .with_detail("x", DetailValue::Float(x + i as f64 * 10.0))
                    .with_detail("y", DetailValue::Float(y + i as f64 * 5.0))
            })
            .collect()
    }

    #[test]
    fn kill_dense_burst_then_pause() {
        // 4 clicks @150ms = 0.45s span, then silence -> kill
        let m = detect_moments(&clicks(10_000.0, 4, 150.0), None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Kill);
        assert!((m[0].timestamp - 10.0).abs() < 1e-9);
        assert!((m[0].duration - 0.45).abs() < 1e-9);
        assert_eq!(m[0].event_count, 4);
        assert!((0.0..=1.0).contains(&m[0].intensity));
    }

    #[test]
    fn hit_small_burst() {
        let m = detect_moments(&clicks(20_000.0, 2, 200.0), None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Hit);
    }

    #[test]
    fn exciting_sustained() {
        let m = detect_moments(&clicks(30_000.0, 10, 300.0), None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Exciting);
    }

    #[test]
    fn lone_click_ignored() {
        assert!(detect_moments(&clicks(50_000.0, 1, 0.0), None).is_empty());
    }

    #[test]
    fn right_click_not_fire() {
        let evs: Vec<InputEvent> = (0..5)
            .map(|i| InputEvent::new(60_000.0 + i as f64 * 100.0, "mouse", "click_right"))
            .collect();
        assert!(detect_moments(&evs, None).is_empty());
    }

    #[test]
    fn gamepad_fire_counts() {
        let evs: Vec<InputEvent> = (0..4)
            .map(|i| {
                InputEvent::new(70_000.0 + i as f64 * 150.0, "gamepad", "btn_down")
                    .with_detail("fire", DetailValue::Bool(true))
            })
            .collect();
        let m = detect_moments(&evs, None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, MomentType::Kill);
    }

    #[test]
    fn gamepad_non_fire_ignored() {
        let evs: Vec<InputEvent> = (0..5)
            .map(|i| {
                InputEvent::new(80_000.0 + i as f64 * 100.0, "gamepad", "btn_down")
                    .with_detail("fire", DetailValue::Bool(false))
            })
            .collect();
        assert!(detect_moments(&evs, None).is_empty());
    }

    #[test]
    fn empty_input() {
        assert!(detect_moments(&[], None).is_empty());
    }

    #[test]
    fn position_centroid_normalized() {
        // clicks around (1440, 810) on 1920x1080 -> ~(0.758, 0.757)
        let m = detect_moments(&clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0), Some((1920.0, 1080.0)));
        assert_eq!(m.len(), 1);
        assert!((m[0].x - 0.758).abs() < 0.01, "x={}", m[0].x);
        assert!((m[0].y - 0.757).abs() < 0.01, "y={}", m[0].y);
    }

    #[test]
    fn position_defaults_to_center() {
        // no screen size -> 0.5, 0.5
        let m = detect_moments(&clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0), None);
        assert_eq!((m[0].x, m[0].y), (0.5, 0.5));
        // gamepad (no position) -> 0.5, 0.5 even with screen size
        let evs: Vec<InputEvent> = (0..4)
            .map(|i| {
                InputEvent::new(70_000.0 + i as f64 * 150.0, "gamepad", "btn_down")
                    .with_detail("fire", DetailValue::Bool(true))
            })
            .collect();
        let m2 = detect_moments(&evs, Some((1920.0, 1080.0)));
        assert_eq!((m2[0].x, m2[0].y), (0.5, 0.5));
    }

    #[test]
    fn position_clamped() {
        let m = Moment::new(1.0, MomentType::Kill, 0.5, 0.4, 4, 2.0, -1.0);
        assert_eq!((m.x, m.y), (1.0, 0.0));
    }

    #[test]
    fn marker_schema() {
        let mk = analyze(&clicks_at(10_000.0, 4, 150.0, 1440.0, 810.0), Some((1920.0, 1080.0)));
        assert_eq!(mk.len(), 1);
        let m = &mk[0];
        assert_eq!(m.kind, "kill");
        assert!((m.timestamp - 10.0).abs() < 1e-9);
        assert!((m.duration - 0.45).abs() < 1e-9);
        assert!((0.0..=1.0).contains(&m.intensity));
        assert!((m.x - 0.758).abs() < 0.01);
        assert!((m.y - 0.757).abs() < 0.01);
    }

    #[test]
    fn output_sorted() {
        let mut evs = clicks(30_000.0, 10, 300.0);
        evs.extend(clicks(10_000.0, 4, 150.0));
        let m = detect_moments(&evs, None);
        let ts: Vec<f64> = m.iter().map(|x| x.timestamp).collect();
        let mut sorted = ts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(ts, sorted);
    }

    #[test]
    fn intensity_clamped() {
        let m = Moment::new(1.0, MomentType::Kill, 5.0, 0.4, 4, 0.5, 0.5);
        assert_eq!(m.intensity, 1.0);
        let m = Moment::new(1.0, MomentType::Kill, -2.0, 0.4, 4, 0.5, 0.5);
        assert_eq!(m.intensity, 0.0);
    }
}
