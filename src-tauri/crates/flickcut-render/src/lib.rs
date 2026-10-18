// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! FlickCut Tauri render pipeline (spike).
//!
//! Turns `Region { start, end, effect, intensity }` values into an ffmpeg
//! filter graph, then renders to MP4 via a bundled static ffmpeg sidecar:
//! - slowmo via `setpts`
//! - zoom via `crop` following the focus point (smooth glide across regions)
//! - curve via piecewise `setpts` speed ramps

pub mod graph;
pub mod render;

pub use graph::{build_filter_graph, FilterGraph};
pub use render::{render_video, RenderError};

/// Highlight effect applied to a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// One edit region on the input timeline (seconds).
///
/// `intensity`: 0..100 slider value (clamped).
/// `focus`: normalized (x, y) zoom target; defaults to frame center.
///   Consecutive zoom regions with different foci glide smoothly between
///   them (same 0.35s smoothstep semantics as the Python engine).
#[derive(Debug, Clone)]
pub struct Region {
    pub start: f64,
    pub end: f64,
    pub effect: Effect,
    pub intensity: f64,
    pub focus: (f64, f64),
}

impl Region {
    pub fn new(
        start: f64,
        end: f64,
        effect: Effect,
        intensity: f64,
    ) -> Result<Self, String> {
        if !(end > start) {
            return Err(format!("end ({end}) must be > start ({start})"));
        }
        Ok(Self {
            start,
            end,
            effect,
            intensity: intensity.clamp(0.0, 100.0),
            focus: (0.5, 0.5),
        })
    }

    pub fn with_focus(mut self, x: f64, y: f64) -> Self {
        self.focus = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        self
    }

    /// Slow-mo factor for an intensity (matches the Python engine).
    pub fn slowmo_factor(&self) -> f64 {
        let i = self.intensity / 100.0;
        (1.0 - i * 0.95).max(0.05)
    }

    /// Zoom amount for an intensity.
    pub fn zoom_amount(&self) -> f64 {
        1.0 + self.intensity / 100.0
    }
}
