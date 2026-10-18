// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! FlickCut edit engine core (spike prototype).
//!
//! Port of the Python `editor.py` region model: a timeline of [`Region`]
//! values (slow motion, speed curve, zoom in/out) evaluated to a playback
//! speed and a zoom (amount + focus center) at any input time `t`.
//!
//! This crate is pure logic (no video I/O): the Tauri backend owns decoding,
//! frame sampling and piping raw frames to the bundled ffmpeg sidecar.

use std::collections::HashMap;

/// Supported highlight effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    SlowMo,
    Curve,
    ZoomIn,
    ZoomOut,
}

impl Effect {
    pub fn as_str(&self) -> &'static str {
        match self {
            Effect::SlowMo => "slowmo",
            Effect::Curve => "curve",
            Effect::ZoomIn => "zoomin",
            Effect::ZoomOut => "zoomout",
        }
    }
}

/// One effect region on the input timeline (seconds). `start < end`.
///
/// Params are effect-specific:
/// - slowmo: `factor` (0.05..1.0 playback speed)
/// - curve: `points` as (time, speed) control points
/// - zoomin/zoomout: `center` (x, y normalized), `zoom_from`, `zoom_to`, `easing`
#[derive(Debug, Clone)]
pub struct Region {
    pub start: f64,
    pub end: f64,
    pub effect: Effect,
    pub params: HashMap<String, Param>,
}

/// Region parameter value.
#[derive(Debug, Clone)]
pub enum Param {
    Float(f64),
    Point(f64, f64),
    Points(Vec<(f64, f64)>),
    Text(String),
}

impl Region {
    pub fn new(
        start: f64,
        end: f64,
        effect: Effect,
        params: HashMap<String, Param>,
    ) -> Result<Self, String> {
        if !(end > start) {
            return Err(format!("end ({end}) must be > start ({start})"));
        }
        Ok(Self { start, end, effect, params })
    }

    pub fn float(&self, key: &str, default: f64) -> f64 {
        match self.params.get(key) {
            Some(Param::Float(v)) => *v,
            _ => default,
        }
    }

    pub fn point(&self, key: &str, default: (f64, f64)) -> (f64, f64) {
        match self.params.get(key) {
            Some(Param::Point(x, y)) => (*x, *y),
            _ => default,
        }
    }
}

pub const MIN_SPEED: f64 = 0.02; // hard floor so the timeline always advances
const CENTER_BLEND: f64 = 0.35; // seconds of smooth focus glide across boundaries

fn ease_smooth(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn ease(u: f64, kind: &str) -> f64 {
    let u = u.clamp(0.0, 1.0);
    match kind {
        "smooth" => ease_smooth(u),
        "in" => u * u,
        "out" => 1.0 - (1.0 - u) * (1.0 - u),
        _ => u, // "linear"
    }
}

/// Playback speed factor at input time `t` (1.0 = normal). Later regions win.
pub fn speed_at(t: f64, regions: &[Region]) -> f64 {
    let mut speed = 1.0;
    for r in regions {
        if !(r.start <= t && t < r.end) {
            continue;
        }
        match r.effect {
            Effect::SlowMo => speed = r.float("factor", 0.25),
            Effect::Curve => {
                let pts = match r.params.get("points") {
                    Some(Param::Points(p)) => p.clone(),
                    _ => vec![(r.start, 1.0), (r.end, 1.0)],
                };
                speed = interp_points(t, &pts);
            }
            Effect::ZoomIn | Effect::ZoomOut => {}
        }
    }
    speed.max(MIN_SPEED)
}

fn interp_points(t: f64, points: &[(f64, f64)]) -> f64 {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if t <= pts[0].0 {
        return pts[0].1;
    }
    if t >= pts[pts.len() - 1].0 {
        return pts[pts.len() - 1].1;
    }
    for w in pts.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t0 <= t && t <= t1 {
            let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            let u = ease_smooth(u);
            return v0 + (v1 - v0) * u;
        }
    }
    pts[pts.len() - 1].1
}

/// Focus center for the active zoom region, gliding smoothly across
/// boundaries with neighboring zoom regions (Recordly-style, no jumps).
fn focus_center_at(t: f64, active: &Region, regions: &[Region]) -> (f64, f64) {
    let (cx, cy) = active.point("center", (0.5, 0.5));
    let mut zooms: Vec<&Region> = regions
        .iter()
        .filter(|r| matches!(r.effect, Effect::ZoomIn | Effect::ZoomOut))
        .collect();
    zooms.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap()
            .then(a.end.partial_cmp(&b.end).unwrap())
    });
    for j in (1..zooms.len()).rev() {
        let (b, a): (&Region, &Region) = (zooms[j], zooms[j - 1]);
        if !std::ptr::eq(a, active) && !std::ptr::eq(b, active) {
            continue;
        }
        if (b.start - CENTER_BLEND) <= t && t <= (b.start + CENTER_BLEND) {
            let (acx, acy) = a.point("center", (0.5, 0.5));
            let (bcx, bcy) = b.point("center", (0.5, 0.5));
            let u = ease_smooth((t - (b.start - CENTER_BLEND)) / (2.0 * CENTER_BLEND));
            return (acx + (bcx - acx) * u, acy + (bcy - acy) * u);
        }
    }
    (cx, cy)
}

/// `(zoom_amount, cx, cy)` at input time `t`, or `None` when no zoom is active.
/// Later regions win on overlap; the center glides across boundaries.
pub fn zoom_at(t: f64, regions: &[Region]) -> Option<(f64, f64, f64)> {
    for r in regions.iter().rev() {
        if !matches!(r.effect, Effect::ZoomIn | Effect::ZoomOut) {
            continue;
        }
        if !(r.start <= t && t < r.end) {
            continue;
        }
        let u = ease((t - r.start) / (r.end - r.start), &easing_of(r));
        let z0 = r.float("zoom_from", 1.0);
        let default_to = if r.effect == Effect::ZoomIn { 1.6 } else { 1.0 };
        let z1 = r.float("zoom_to", default_to);
        let (cx, cy) = focus_center_at(t, r, regions);
        return Some((z0 + (z1 - z0) * u, cx, cy));
    }
    None
}

fn easing_of(r: &Region) -> String {
    match r.params.get("easing") {
        Some(Param::Text(s)) => s.clone(),
        _ => "smooth".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Interactive region tools (UI slider round-trip)
// ---------------------------------------------------------------------------

fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

/// Set a region's strength from a 0..100 slider value (mutates params in place).
pub fn set_region_intensity(region: &mut Region, intensity: f64) {
    let i = clamp01(intensity / 100.0);
    match region.effect {
        Effect::SlowMo => {
            region.params.insert(
                "factor".to_string(),
                Param::Float((1.0 - i * 0.95).max(0.05)),
            );
        }
        Effect::Curve => {
            let fmin = (1.0 - i * 0.95).max(0.05);
            let mid = (region.start + region.end) / 2.0;
            region.params.insert(
                "points".to_string(),
                Param::Points(vec![(region.start, 1.0), (mid, fmin), (region.end, 1.0)]),
            );
        }
        Effect::ZoomIn => {
            let amount = 1.0 + i;
            region.params.insert("zoom_from".to_string(), Param::Float(1.0));
            region.params.insert("zoom_to".to_string(), Param::Float(amount));
        }
        Effect::ZoomOut => {
            let amount = 1.0 + i;
            region.params.insert("zoom_from".to_string(), Param::Float(amount));
            region.params.insert("zoom_to".to_string(), Param::Float(1.0));
        }
    }
}

/// Inverse of `set_region_intensity`: read back 0..100 (clamped).
pub fn region_intensity(region: &Region) -> f64 {
    let i = match region.effect {
        Effect::SlowMo => (1.0 - region.float("factor", 1.0)) / 0.95,
        Effect::Curve => {
            let fmin = match region.params.get("points") {
                Some(Param::Points(pts)) => {
                    pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min)
                }
                _ => 1.0,
            };
            (1.0 - fmin) / 0.95
        }
        Effect::ZoomIn => region.float("zoom_to", 1.0) - 1.0,
        Effect::ZoomOut => region.float("zoom_from", 1.0) - 1.0,
    };
    clamp01(i) * 100.0
}

/// Short English UI label, e.g. "Slow-mo 0.8s-2.0s (4x)".
pub fn describe_region(region: &Region) -> String {
    let label = match region.effect {
        Effect::SlowMo => "Slow-mo",
        Effect::Curve => "Speed curve",
        Effect::ZoomIn => "Zoom-in",
        Effect::ZoomOut => "Zoom-out",
    };
    let span = format!("{:.1}s-{:.1}s", region.start, region.end);
    let fmt = |v: f64| {
        if v.fract() == 0.0 {
            format!("{v:.0}")
        } else {
            format!("{v:.1}")
        }
    };
    match region.effect {
        Effect::SlowMo => {
            let f = region.float("factor", 1.0);
            let m = if f > 0.0 { 1.0 / f } else { 0.0 };
            format!("{label} {span} ({}x)", fmt(m))
        }
        Effect::Curve => {
            let fmin = match region.params.get("points") {
                Some(Param::Points(pts)) => {
                    pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min)
                }
                _ => 1.0,
            };
            let m = if fmin > 0.0 { 1.0 / fmin } else { 0.0 };
            format!("{label} {span} ({}x dip)", fmt(m))
        }
        Effect::ZoomIn => {
            format!("{label} {span} ({}x)", fmt(region.float("zoom_to", 1.0)))
        }
        Effect::ZoomOut => {
            format!("{label} {span} ({}x)", fmt(region.float("zoom_from", 1.0)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(effect: Effect) -> Region {
        Region::new(1.0, 3.0, effect, HashMap::new()).unwrap()
    }

    #[test]
    fn intensity_round_trip_all_effects() {
        for effect in [Effect::SlowMo, Effect::Curve, Effect::ZoomIn, Effect::ZoomOut] {
            for v in [0.0, 1.0, 25.0, 50.0, 75.0, 99.0, 100.0] {
                let mut r = region(effect);
                set_region_intensity(&mut r, v);
                let back = region_intensity(&r);
                assert!((back - v).abs() < 1e-9, "{effect:?} {v} -> {back}");
            }
        }
    }

    #[test]
    fn intensity_clamps() {
        let mut r = region(Effect::SlowMo);
        set_region_intensity(&mut r, 150.0);
        assert_eq!(region_intensity(&r), 100.0);
        set_region_intensity(&mut r, -20.0);
        assert_eq!(region_intensity(&r), 0.0);
    }

    #[test]
    fn describe_formats() {
        let mut r = Region::new(0.8, 2.0, Effect::SlowMo, HashMap::new()).unwrap();
        r.params.insert("factor".to_string(), Param::Float(0.25));
        assert_eq!(describe_region(&r), "Slow-mo 0.8s-2.0s (4x)");
        let mut z = Region::new(1.2, 2.2, Effect::ZoomIn, HashMap::new()).unwrap();
        z.params.insert("zoom_to".to_string(), Param::Float(1.6));
        assert_eq!(describe_region(&z), "Zoom-in 1.2s-2.2s (1.6x)");
    }

    #[test]
    fn focus_glides_across_boundary() {
        let mut pa = HashMap::new();
        pa.insert("center".to_string(), Param::Point(0.2, 0.5));
        let mut pb = HashMap::new();
        pb.insert("center".to_string(), Param::Point(0.8, 0.5));
        let a = Region::new(1.0, 2.0, Effect::ZoomIn, pa).unwrap();
        let b = Region::new(2.0, 3.0, Effect::ZoomIn, pb).unwrap();
        let regions = vec![a, b];
        let mut prev_x = 0.2;
        let mut t = 1.6;
        while t <= 2.41 {
            let (_, cx, _) = zoom_at(t, &regions).expect("zoom active");
            assert!((cx - prev_x).abs() < 0.05, "jump at t={t}: {prev_x} -> {cx}");
            assert!(cx + 1e-9 >= prev_x, "non-monotonic at t={t}");
            prev_x = cx;
            t += 0.01;
        }
        assert!((prev_x - 0.8).abs() < 0.01);
    }

    #[test]
    fn single_region_center_unchanged() {
        let mut p = HashMap::new();
        p.insert("center".to_string(), Param::Point(0.3, 0.7));
        let r = Region::new(1.0, 3.0, Effect::ZoomIn, p).unwrap();
        for t in [1.0, 1.5, 2.0, 2.99] {
            let (_, cx, cy) = zoom_at(t, &[r.clone()]).unwrap();
            assert!((cx - 0.3).abs() < 1e-12);
            assert!((cy - 0.7).abs() < 1e-12);
        }
    }

    #[test]
    fn speed_at_slowmo() {
        let mut p = HashMap::new();
        p.insert("factor".to_string(), Param::Float(0.25));
        let r = Region::new(1.0, 2.0, Effect::SlowMo, p).unwrap();
        assert_eq!(speed_at(0.5, &[r.clone()]), 1.0);
        assert_eq!(speed_at(1.5, &[r.clone()]), 0.25);
        assert_eq!(speed_at(2.5, &[r]), 1.0);
    }
}
