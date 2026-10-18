// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Recording sessions (`start_recording` / `stop_recording`).
//!
//! The platform [`Backend`] pushes BGRA frames into a channel; one worker
//! thread drains it into an `ffmpeg` subprocess that encodes MP4 (libx264).
//! Stopping is cooperative: the backend is stopped, the worker is joined with
//! a timeout, and the encoder is killed as a last resort so callers never hang.
//!
//! The session clock ([`crate::clock::SessionClock`]) is owned by the
//! [`Session`] and shared (via [`Session::clock`]) with input telemetry so
//! kill moments line up with video frames.

use crate::backends::{create_backend, Backend};
use crate::clock::SessionClock;
use crate::frame::Frame;
use crossbeam_channel::Receiver;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Errors from the capture pipeline.
#[derive(Debug)]
pub enum CaptureError {
    /// Platform backend cannot run here (e.g. missing OS API).
    UnsupportedPlatform(String),
    /// OS did not grant screen-recording permission.
    NoPermission,
    /// No such source id (see [`crate::sources::list_sources`]).
    UnknownSource(String),
    /// The backend failed to build or run the capturer.
    CaptureFailed(String),
    /// The ffmpeg encoder subprocess failed.
    EncoderFailed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform(s) => write!(f, "platform not supported: {s}"),
            Self::NoPermission => write!(f, "screen recording permission not granted"),
            Self::UnknownSource(id) => write!(f, "unknown source: {id}"),
            Self::CaptureFailed(s) => write!(f, "capture failed: {s}"),
            Self::EncoderFailed(s) => write!(f, "encoder failed: {s}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// The `stop_recording()` result.
#[derive(Debug, Clone)]
pub struct RecordingSummary {
    pub session_id: String,
    pub video_path: PathBuf,
    pub duration_sec: f64,
    pub frames_captured: u64,
    pub frames_written: u64,
    pub frames_dropped: u64,
    pub width: u32,
    pub height: u32,
    pub fps_target: u32,
}

/// An active recording session. Owns the session clock.
pub struct Session {
    id: String,
    clock: SessionClock,
    output_path: PathBuf,
    fps: u32,
    backend: Box<dyn Backend>,
    worker: Option<JoinHandle<WorkerOutcome>>,
    encoder: Option<Child>,
    width: u32,
    height: u32,
}

struct WorkerOutcome {
    frames_written: u64,
    frames_dropped: u64,
    width: u32,
    height: u32,
    error: Option<String>,
}

impl Session {
    /// Start recording `source_id` (see [`crate::sources::list_sources`])
    /// at `fps`, encoding to `output_path` (MP4).
    ///
    /// The session clock starts now; share [`Session::clock`] with input
    /// telemetry before the first event is stamped.
    pub fn start(source_id: &str, fps: u32, output_path: &Path) -> Result<Self, CaptureError> {
        let mut backend = create_backend();
        let frames_rx = backend.start(source_id, fps)?;

        // Wait for the first frame to learn the real dimensions before
        // spawning the encoder. Generous timeout: on Linux the OS picker
        // needs human interaction; cancellation fails fast via channel close.
        let first = frames_rx
            .recv_timeout(Duration::from_secs(120))
            .map_err(|_| {
                backend.stop();
                CaptureError::CaptureFailed(
                    "no frames arrived (source picker cancelled or backend failed)".into(),
                )
            })?;
        let (w, h) = (first.width, first.height);

        let mut encoder =
            spawn_encoder(output_path, w, h, fps).map_err(CaptureError::EncoderFailed)?;
        let stdin = encoder
            .stdin
            .take()
            .ok_or_else(|| CaptureError::EncoderFailed("could not open encoder stdin".into()))?;

        let clock = SessionClock::new();
        let id = format!("sess-{}", (clock.t0_wall() * 1000.0) as u64);

        // Move the receiver into the worker: a single consumer, so no frame
        // is ever routed to a dead receiver.
        let worker = thread::Builder::new()
            .name("flickcut-encode".into())
            .spawn(move || run_worker(frames_rx, stdin, first))
            .map_err(|e| CaptureError::CaptureFailed(format!("spawn worker: {e}")))?;

        Ok(Self {
            id,
            clock,
            output_path: output_path.to_path_buf(),
            fps,
            backend,
            worker: Some(worker),
            encoder: Some(encoder),
            width: w,
            height: h,
        })
    }

    /// This session's clock. Clone it and hand it to input telemetry;
    /// `clock.stamp()` on both sides yields comparable `t_mono` values.
    pub fn clock(&self) -> SessionClock {
        self.clock.clone()
    }

    /// The session id (`start_recording` return value).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Stop recording and finalize the video file.
    ///
    /// Always returns (never hangs): the backend is stopped, the worker is
    /// joined with a timeout, and the encoder is killed as a last resort.
    pub fn stop(mut self) -> Result<RecordingSummary, CaptureError> {
        // 1. Stop the backend → its worker exits → frame channel closes.
        self.backend.stop();

        // 2. Join the encoder worker (it exits on channel close / stdin EOF).
        let outcome = match self.worker.take() {
            Some(h) => {
                let (tx, rx) = std::sync::mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(h.join());
                });
                match rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(Ok(o)) => o,
                    Ok(Err(_)) => WorkerOutcome {
                        frames_written: 0,
                        frames_dropped: 0,
                        width: self.width,
                        height: self.height,
                        error: Some("encoder thread panicked".into()),
                    },
                    Err(_) => {
                        if let Some(mut enc) = self.encoder.take() {
                            let _ = enc.kill();
                            let _ = enc.wait();
                        }
                        WorkerOutcome {
                            frames_written: 0,
                            frames_dropped: 0,
                            width: self.width,
                            height: self.height,
                            error: Some("encoder thread did not stop in time".into()),
                        }
                    }
                }
            }
            None => WorkerOutcome {
                frames_written: 0,
                frames_dropped: 0,
                width: self.width,
                height: self.height,
                error: Some("no worker thread".into()),
            },
        };

        // 3. Wait for ffmpeg to finalize the MP4.
        if let Some(mut enc) = self.encoder.take() {
            match enc.wait() {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    return Err(CaptureError::EncoderFailed(format!(
                        "ffmpeg exited with {status}"
                    )))
                }
                Err(e) => return Err(CaptureError::EncoderFailed(format!("wait: {e}"))),
            }
        }
        if let Some(e) = outcome.error {
            return Err(CaptureError::CaptureFailed(e));
        }

        Ok(RecordingSummary {
            session_id: self.id.clone(),
            video_path: self.output_path.clone(),
            duration_sec: self.clock.elapsed_sec(),
            frames_captured: outcome.frames_written + outcome.frames_dropped,
            frames_written: outcome.frames_written,
            frames_dropped: outcome.frames_dropped,
            width: outcome.width,
            height: outcome.height,
            fps_target: self.fps,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Never leave a session recording on drop: best-effort stop.
        self.backend.stop();
        if let Some(mut enc) = self.encoder.take() {
            let _ = enc.kill();
        }
    }
}

/// Spawn `ffmpeg` reading raw BGRA frames from stdin and writing MP4.
fn spawn_encoder(output: &Path, w: u32, h: u32, fps: u32) -> Result<Child, String> {
    Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "bgra",
            "-s",
            &format!("{w}x{h}"),
            "-framerate",
            &fps.to_string(),
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-movflags",
            "+faststart",
        ])
        .arg(output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn ffmpeg: {e} (is ffmpeg installed?)"))
}

/// Worker: drain backend frames (first frame passed in) into ffmpeg stdin.
fn run_worker(rx: Receiver<Frame>, mut stdin: ChildStdin, first: Frame) -> WorkerOutcome {
    let mut frames_written: u64 = 0;
    let mut frames_dropped: u64 = 0;
    let mut error: Option<String> = None;
    let (mut w, mut h) = (first.width, first.height);

    // The backend channel is bounded with try_send → overflow counts as drops.
    // Here we count a drop when a frame arrives with unexpected dimensions.
    let mut pending: Option<Frame> = Some(first);
    loop {
        let frame = match pending.take() {
            Some(f) => f,
            None => match rx.recv() {
                Ok(f) => f,
                Err(_) => break, // backend stopped → channel closed
            },
        };
        if frame.width != w || frame.height != h {
            // Mid-stream resize: keep the encoder's dimensions, drop the frame.
            frames_dropped += 1;
            continue;
        }
        if frame.data.len() != w as usize * h as usize * 4 {
            frames_dropped += 1;
            continue;
        }
        if stdin.write_all(&frame.data).is_err() {
            error = Some("encoder stdin closed".into());
            break;
        }
        frames_written += 1;
        let _ = (w, h);
    }

    drop(stdin); // EOF → ffmpeg finalizes the file
    WorkerOutcome {
        frames_written,
        frames_dropped,
        width: w,
        height: h,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn unknown_source_is_rejected() {
        let r = Session::start("bogus:999", 60, Path::new("/tmp/nope.mp4"));
        assert!(matches!(r, Err(CaptureError::UnknownSource(_))));
    }

    #[test]
    fn malformed_source_is_rejected() {
        let r = Session::start("definitely-not-a-source", 60, Path::new("/tmp/nope.mp4"));
        assert!(matches!(r, Err(CaptureError::UnknownSource(_))));
    }
}
