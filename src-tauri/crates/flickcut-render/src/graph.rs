// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! ffmpeg filter-graph generation from edit regions.
//!
//! Strategy: split the timeline at region boundaries into pieces, apply each
//! piece's effect with trim/setpts/crop, then concat. All timestamps are
//! re-based per piece (`setpts=PTS-STARTPTS`) so per-frame expressions use
//! local time `t` starting at 0.
//!
//! - slowmo: `setpts=MULT*PTS` (MULT = 1/factor)
//! - curve: split into ramp pieces, each with its own constant `setpts`
//! - zoom: `crop` following the focus point (+ smooth glide across regions),
//!   then `scale` back to full size

use crate::{Effect, Region};

/// Seconds of smooth focus glide centered on a zoom-region boundary.
pub const CENTER_BLEND: f64 = 0.35;
/// Pieces used to approximate a speed-curve ramp.
const CURVE_STEPS: usize = 8;

/// A built filter graph ready for `-filter_complex`.
#[derive(Debug, Clone)]
pub struct FilterGraph {
    /// The `-filter_complex` script.
    pub filter_complex: String,
    /// Output label to map, e.g. `[outv]`.
    pub output_label: String,
    /// Number of concatenated pieces (for progress estimation).
    pub pieces: usize,
}

/// Internal timeline piece with its resolved effect.
enum PieceKind<'a> {
    Passthrough,
    SlowMo { mult: f64 },
    Zoom { region: &'a Region },
}

struct Piece<'a> {
    start: f64,
    end: f64,
    kind: PieceKind<'a>,
}

fn fmt_ts(v: f64) -> String {
    format!("{v:.3}")
}

/// smoothstep(clip(x,0,1)) as an ffmpeg expr fragment.
fn sstep(var: &str) -> String {
    format!("({var})*({var})*(3-2*({var}))")
}

fn clip01(expr: &str) -> String {
    format!("clip({expr},0,1)")
}

/// Zoom amount expression over local time `t` in a piece of length `d`.
fn zoom_expr(z0: f64, z1: f64, d: f64) -> String {
    let su = sstep(&clip01(&format!("t/{d:.3}")));
    format!("{z0:.4}+({z1:.4}-{z0:.4})*{su}")
}

/// Focus coordinate expression for one axis.
///
/// Blends from `own` toward `neighbor` with smoothstep across the global
/// window `[boundary - CENTER_BLEND, boundary + CENTER_BLEND]`, expressed in
/// this piece's local time (`t = 0` at `seg_start`).
fn focus_expr(own: f64, neighbor: f64, boundary: f64, seg_start: f64) -> String {
    if (neighbor - own).abs() < 1e-9 {
        return format!("{own:.4}");
    }
    let t0 = boundary - CENTER_BLEND - seg_start;
    let su = sstep(&clip01(&format!("(t-{t0:.3})/{:.3}", 2.0 * CENTER_BLEND)));
    format!("{own:.4}+({neighbor:.4}-{own:.4})*{su}")
}

/// Speed at fraction `u` (0..1) through a curve region (matches Python:
/// smoothstep dip to fmin at the middle).
fn curve_speed_at(intensity: f64, u: f64) -> f64 {
    let i = (intensity / 100.0).clamp(0.0, 1.0);
    let fmin = (1.0 - i * 0.95).max(0.05);
    let d = (u - 0.5).abs() * 2.0; // 0 at middle, 1 at ends
    let s = d * d * (3.0 - 2.0 * d);
    fmin + (1.0 - fmin) * s
}

/// Build the crop filter for a zoom piece, following the focus point with
/// smooth glide toward neighboring regions' foci at boundaries.
fn zoom_filter(r: &Region, seg_start: f64, seg_end: f64, regions: &[Region]) -> String {
    let d = seg_end - seg_start;
    let amount = r.zoom_amount();
    let (z0, z1) = match r.effect {
        Effect::ZoomIn => (1.0, amount),
        Effect::ZoomOut => (amount, 1.0),
        _ => (1.0, 1.0),
    };
    let z = zoom_expr(z0, z1, d);

    // Neighboring zoom regions (by start order) for the glide.
    let mut zooms: Vec<&Region> = regions
        .iter()
        .filter(|x| matches!(x.effect, Effect::ZoomIn | Effect::ZoomOut))
        .collect();
    zooms.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    let pos = zooms.iter().position(|x| std::ptr::eq(*x, r));

    // Prefer gliding out toward the next region (the visible handoff is at
    // this piece's end); otherwise glide in from the previous region.
    let (cx, cy) = match pos {
        Some(j) if j + 1 < zooms.len() && zooms[j + 1].focus != r.focus => {
            let n = zooms[j + 1];
            (
                focus_expr(r.focus.0, n.focus.0, n.start, seg_start),
                focus_expr(r.focus.1, n.focus.1, n.start, seg_start),
            )
        }
        Some(j) if j > 0 && zooms[j - 1].focus != r.focus => {
            let p = zooms[j - 1];
            (
                focus_expr(r.focus.0, p.focus.0, r.start, seg_start),
                focus_expr(r.focus.1, p.focus.1, r.start, seg_start),
            )
        }
        _ => (format!("{:.4}", r.focus.0), format!("{:.4}", r.focus.1)),
    };

    format!(
        "crop=w='iw/({z})':h='ih/({z})':x='(iw-ow)*({cx})':y='(ih-oh)*({cy})',scale=iw:ih"
    )
}

/// Build the full `-filter_complex` script.
///
/// `duration` bounds the timeline; `fps` is accepted for future
/// frame-accurate trims (currently unused). Overlapping regions are an
/// error — the filter graph needs clean segments.
pub fn build_filter_graph(
    regions: &[Region],
    duration: f64,
    fps: f64,
) -> Result<FilterGraph, String> {
    if duration <= 0.0 {
        return Err("duration must be > 0".to_string());
    }
    if fps <= 0.0 {
        return Err("fps must be > 0".to_string());
    }
    let mut sorted: Vec<Region> = regions.to_vec();
    sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    for w in sorted.windows(2) {
        if w[1].start < w[0].end - 1e-9 {
            return Err(format!(
                "overlapping regions: [{:.2},{:.2}] and [{:.2},{:.2}]",
                w[0].start, w[0].end, w[1].start, w[1].end
            ));
        }
    }

    // Cut points: 0, duration, every region start/end.
    let mut cuts = vec![0.0, duration];
    for r in &sorted {
        cuts.push(r.start.max(0.0));
        cuts.push(r.end.min(duration));
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    // Build pieces; curve regions expand into ramp sub-pieces.
    let mut pieces: Vec<Piece> = Vec::new();
    for w in cuts.windows(2) {
        let (s, e) = (w[0], w[1]);
        if e - s < 1e-9 {
            continue;
        }
        let mid = (s + e) / 2.0;
        let owner = sorted.iter().find(|r| r.start <= mid && mid < r.end);
        let kind = match owner {
            None => PieceKind::Passthrough,
            Some(r) => match r.effect {
                Effect::SlowMo => PieceKind::SlowMo { mult: 1.0 / r.slowmo_factor() },
                Effect::ZoomIn | Effect::ZoomOut => PieceKind::Zoom { region: r },
                Effect::Curve => {
                    // Subdivide this piece further into ramp steps. The
                    // outer cut already isolated the curve region, so split
                    // [s, e] directly.
                    for k in 0..CURVE_STEPS {
                        let ks = s + (e - s) * k as f64 / CURVE_STEPS as f64;
                        let ke = s + (e - s) * (k + 1) as f64 / CURVE_STEPS as f64;
                        // u measured against the *region*, not the piece
                        let u_mid = ((ks + ke) / 2.0 - r.start) / (r.end - r.start);
                        let speed = curve_speed_at(r.intensity, u_mid).max(0.05);
                        pieces.push(Piece {
                            start: ks,
                            end: ke,
                            kind: PieceKind::SlowMo { mult: 1.0 / speed },
                        });
                    }
                    continue;
                }
            },
        };
        pieces.push(Piece { start: s, end: e, kind });
    }

    let mut parts: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for (idx, p) in pieces.iter().enumerate() {
        let label = format!("[v{idx}]");
        let mut chain = format!(
            "[0:v]trim=start={}:end={},setpts=PTS-STARTPTS",
            fmt_ts(p.start),
            fmt_ts(p.end)
        );
        match &p.kind {
            PieceKind::Passthrough => {}
            PieceKind::SlowMo { mult } => {
                chain.push_str(&format!(",setpts={mult:.4}*PTS"));
            }
            PieceKind::Zoom { region } => {
                chain.push(',');
                chain.push_str(&zoom_filter(region, p.start, p.end, &sorted));
            }
        }
        chain.push_str(&label);
        parts.push(chain);
        labels.push(label);
    }

    let concat = format!(
        "{}concat=n={}:v=1:a=0[outv]",
        labels.join(""),
        pieces.len()
    );
    parts.push(concat);
    Ok(FilterGraph {
        filter_complex: parts.join(";\n"),
        output_label: "[outv]".to_string(),
        pieces: pieces.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Effect;

    fn reg(s: f64, e: f64, fx: Effect, i: f64) -> Region {
        Region::new(s, e, fx, i).unwrap()
    }

    #[test]
    fn slowmo_setpts() {
        let g = build_filter_graph(&[reg(1.0, 2.0, Effect::SlowMo, 75.0)], 5.0, 30.0).unwrap();
        // intensity 75 -> factor 0.2875 -> setpts 3.4783*PTS
        assert!(g.filter_complex.contains("setpts=3.4783*PTS"), "{}", g.filter_complex);
        assert!(g.filter_complex.contains("trim=start=1.000:end=2.000"));
        assert_eq!(g.pieces, 3); // [0,1] [1,2] [2,5]
    }

    #[test]
    fn zoom_crop_follows_focus() {
        let r = reg(1.0, 2.0, Effect::ZoomIn, 60.0).with_focus(0.2, 0.8);
        let g = build_filter_graph(&[r], 5.0, 30.0).unwrap();
        assert!(g.filter_complex.contains("crop="), "{}", g.filter_complex);
        assert!(g.filter_complex.contains("0.2000"), "{}", g.filter_complex);
        assert!(g.filter_complex.contains("0.8000"), "{}", g.filter_complex);
        assert!(
            g.filter_complex.contains("1.0000+(1.6000-1.0000)"),
            "{}",
            g.filter_complex
        );
    }

    #[test]
    fn zoom_glide_between_regions() {
        let a = reg(1.0, 2.0, Effect::ZoomIn, 60.0).with_focus(0.2, 0.5);
        let b = reg(2.0, 3.0, Effect::ZoomIn, 60.0).with_focus(0.8, 0.5);
        let g = build_filter_graph(&[a, b], 5.0, 30.0).unwrap();
        assert!(
            g.filter_complex.contains("0.2000+(0.8000-0.2000)"),
            "{}",
            g.filter_complex
        );
    }

    #[test]
    fn curve_ramps() {
        let g = build_filter_graph(&[reg(1.0, 3.0, Effect::Curve, 80.0)], 5.0, 30.0).unwrap();
        assert_eq!(g.pieces, 2 + CURVE_STEPS);
        assert!(g.filter_complex.contains("setpts="));
        // middle pieces slower -> larger multiplier than edge pieces
        assert!(g.filter_complex.contains("setpts=4.0000*PTS") || g.filter_complex.contains("setpts=3."),
            "{}", g.filter_complex);
    }

    #[test]
    fn concat_structure() {
        let g = build_filter_graph(
            &[
                reg(1.0, 2.0, Effect::SlowMo, 50.0),
                reg(3.0, 4.0, Effect::ZoomIn, 50.0),
            ],
            5.0,
            30.0,
        )
        .unwrap();
        assert!(g.filter_complex.contains("[v0][v1][v2][v3][v4]concat=n=5:v=1:a=0[outv]"));
    }

    #[test]
    fn rejects_overlap() {
        let r = build_filter_graph(
            &[
                reg(1.0, 3.0, Effect::SlowMo, 50.0),
                reg(2.0, 4.0, Effect::SlowMo, 50.0),
            ],
            5.0,
            30.0,
        );
        assert!(r.is_err());
    }

    #[test]
    fn empty_regions_passthrough() {
        let g = build_filter_graph(&[], 5.0, 30.0).unwrap();
        assert_eq!(g.pieces, 1);
        assert!(g.filter_complex.contains("concat=n=1"));
    }
}
