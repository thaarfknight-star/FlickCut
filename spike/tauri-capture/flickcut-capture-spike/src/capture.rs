//! Unified capture model for the FlickCut Tauri v2 spike.
//!
//! Mirrors the Python recorder's contract: every frame carries a wall-clock
//! timestamp (`t_wall`, for aligning kill moments) and a monotonic timestamp
//! (`t_mono`, for pacing/fps math). The Tauri backend would expose these over
//! IPC; frames themselves are encoded in Rust (never shipped raw to the
//! webview).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// A capture target: a display or a window.
#[derive(Debug, Clone)]
pub struct CaptureTarget {
    pub kind: TargetKind,
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Monitor,
    Window,
}

/// One captured frame: RGBA pixels plus precise timestamps.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Wall-clock seconds since the Unix epoch (aligns with input telemetry).
    pub t_wall: f64,
    /// Monotonic seconds since an arbitrary point (pacing/fps math).
    pub t_mono: f64,
    pub pixels_rgba: Vec<u8>,
}

pub fn now_wall() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn now_mono() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

// ---------------------------------------------------------------------------
// Linux backend (xcap). Compiled and exercised on this host.
// Windows (windows-capture) and macOS (screencapturekit) backends implement
// the same two functions behind cfg(target_os); see SPIKE_REPORT.md.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub fn list_targets() -> Result<Vec<CaptureTarget>, String> {
    use xcap::Monitor;
    let monitors = Monitor::all().map_err(|e| format!("Monitor::all: {e}"))?;
    Ok(monitors
        .into_iter()
        .enumerate()
        .map(|(i, m)| CaptureTarget {
            kind: TargetKind::Monitor,
            id: i as u32,
            name: m.name().unwrap_or_else(|_| format!("monitor-{i}")),
            x: m.x().unwrap_or(0),
            y: m.y().unwrap_or(0),
            width: m.width().unwrap_or(0),
            height: m.height().unwrap_or(0),
        })
        .collect())
}

#[cfg(target_os = "linux")]
pub fn capture_frame(target_id: u32) -> Result<Frame, String> {
    use xcap::Monitor;
    let monitors = Monitor::all().map_err(|e| format!("Monitor::all: {e}"))?;
    let mon = monitors
        .get(target_id as usize)
        .ok_or_else(|| format!("no monitor with id {target_id}"))?;
    let t_wall = now_wall();
    let t_mono = now_mono();
    let img = mon
        .capture_image()
        .map_err(|e| format!("capture_image: {e}"))?;
    let (w, h) = (img.width(), img.height());
    Ok(Frame {
        width: w,
        height: h,
        t_wall,
        t_mono,
        pixels_rgba: img.into_raw(),
    })
}

#[cfg(not(target_os = "linux"))]
pub fn list_targets() -> Result<Vec<CaptureTarget>, String> {
    Err("this spike binary only implements the Linux backend; \
         Windows/macOS backends are documented in SPIKE_REPORT.md"
        .to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn capture_frame(_target_id: u32) -> Result<Frame, String> {
    Err("this spike binary only implements the Linux backend".to_string())
}
