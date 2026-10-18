// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Render contract: `render_video(video_path, regions)` -> output path.
//!
//! Builds the ffmpeg filter graph (see `graph`), spawns the bundled static
//! ffmpeg sidecar, parses its progress from stderr, and forwards it through
//! the progress callback. In the Tauri app the callback emits
//! `render-progress` events to the frontend.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::graph::build_filter_graph;
use crate::Region;

/// Render errors.
#[derive(Debug)]
pub enum RenderError {
    Graph(String),
    Spawn(String),
    Failed(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Graph(e) => write!(f, "filter graph: {e}"),
            RenderError::Spawn(e) => write!(f, "spawn ffmpeg: {e}"),
            RenderError::Failed(e) => write!(f, "ffmpeg failed: {e}"),
        }
    }
}

/// Probe video duration/fps via the sidecar ffmpeg (`-i` stderr parse).
/// Kept dependency-free: parses the `Duration:` / `Video:` lines.
pub fn probe(ffmpeg: &str, video_path: &str) -> Result<(f64, f64), RenderError> {
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-i", video_path])
        .output()
        .map_err(|e| RenderError::Spawn(e.to_string()))?;
    let err = String::from_utf8_lossy(&out.stderr);
    let duration = parse_duration(&err).ok_or_else(|| {
        RenderError::Failed(format!("could not parse duration for {video_path}"))
    })?;
    let fps = parse_fps(&err).unwrap_or(30.0);
    Ok((duration, fps))
}

fn parse_duration(stderr: &str) -> Option<f64> {
    // "Duration: 00:01:23.45, ..."
    let line = stderr.lines().find(|l| l.contains("Duration:"))?;
    let part = line.split("Duration:").nth(1)?.split(',').next()?.trim();
    let mut it = part.split(':');
    let h: f64 = it.next()?.parse().ok()?;
    let m: f64 = it.next()?.parse().ok()?;
    let s: f64 = it.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

fn parse_fps(stderr: &str) -> Option<f64> {
    // "... Video: h264 ..., 30 fps, ..."
    let line = stderr.lines().find(|l| l.contains("Video:"))?;
    let part = line.split("fps").next()?;
    part.rsplit(&[' ', ','][..]).find_map(|tok| tok.parse::<f64>().ok())
}

/// Build the exact ffmpeg argv for a render (testable without a binary).
pub fn ffmpeg_argv(
    ffmpeg: &str,
    video_path: &str,
    filter_complex: &str,
    output_path: &str,
) -> Vec<String> {
    vec![
        ffmpeg.to_string(),
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        video_path.to_string(),
        "-filter_complex".to_string(),
        filter_complex.to_string(),
        "-map".to_string(),
        "[outv]".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-progress".to_string(),
        "pipe:2".to_string(),
        output_path.to_string(),
    ]
}

/// Render `video_path` with `regions` to `output_path`.
///
/// `ffmpeg` is the path to the bundled static sidecar binary.
/// `on_progress` receives 0..100 (in Tauri: forwarded as `render-progress`
/// events). Returns the output path on success.
pub fn render_video(
    ffmpeg: &str,
    video_path: &str,
    regions: &[Region],
    output_path: &str,
    on_progress: &dyn Fn(u8),
) -> Result<String, RenderError> {
    let (duration, fps) = probe(ffmpeg, video_path)?;
    let graph =
        build_filter_graph(regions, duration, fps).map_err(RenderError::Graph)?;
    let argv = ffmpeg_argv(ffmpeg, video_path, &graph.filter_complex, output_path);

    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| RenderError::Spawn(e.to_string()))?;

    let stderr = child.stderr.take().expect("piped");
    let reader = BufReader::new(stderr);
    let mut out_ms: Option<f64> = None;
    for line in reader.lines().map_while(Result::ok) {
        // ffmpeg -progress key=value lines; out_time_ms drives progress
        if let Some(v) = line.strip_prefix("out_time_ms=") {
            if let Ok(ms) = v.trim().parse::<f64>() {
                out_ms = Some(ms);
            }
        }
        if line.trim() == "progress=continue" {
            if let Some(ms) = out_ms {
                let pct = ((ms / 1000.0 / duration) * 100.0).clamp(0.0, 100.0) as u8;
                on_progress(pct);
            }
        }
    }
    let status = child.wait().map_err(|e| RenderError::Spawn(e.to_string()))?;
    if !status.success() {
        return Err(RenderError::Failed(format!("exit {status}")));
    }
    on_progress(100);
    Ok(output_path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_shape() {
        let argv = ffmpeg_argv("/bin/ffmpeg", "in.mp4", "[0:v]null[outv]", "out.mp4");
        assert_eq!(argv[0], "/bin/ffmpeg");
        assert!(argv.contains(&"-filter_complex".to_string()));
        let fi = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert_eq!(argv[fi + 1], "[0:v]null[outv]");
        assert!(argv.contains(&"[outv]".to_string()));
        assert!(argv.contains(&"libx264".to_string()));
        assert_eq!(argv.last().unwrap(), "out.mp4");
    }

    #[test]
    fn parses_duration() {
        let err = "  Duration: 00:01:23.45, start: 0.000000, bitrate: 123 kb/s";
        assert!((parse_duration(err).unwrap() - 83.45).abs() < 1e-9);
    }

    #[test]
    fn parses_fps() {
        let err = "    Stream #0:0: Video: h264, yuv420p, 1920x1080, 30 fps, 30 tbr";
        assert_eq!(parse_fps(err).unwrap(), 30.0);
    }

    #[test]
    fn parse_garbage_none() {
        assert!(parse_duration("nothing here").is_none());
        assert!(parse_fps("nothing here").is_none());
    }
}
