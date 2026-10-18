//! Recording sessions (`start_recording` / `stop_recording`).
//!
//! One background worker thread runs the scap capture loop and pipes raw
//! BGRA frames straight into an `ffmpeg` subprocess that encodes MP4
//! (libx264). Stopping is cooperative via an atomic flag; `stop()` joins the
//! worker with a timeout and kills the encoder as a last resort so the UI
//! thread can never hang.
//!
//! The session clock ([`crate::clock::SessionClock`]) is owned by the
//! [`Session`] and shared (via [`Session::clock`]) with input telemetry so
//! kill moments line up with video frames.

use crate::clock::SessionClock;
use crate::sources::{resolve_target, ResolvedTarget};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Errors from the capture pipeline.
#[derive(Debug)]
pub enum CaptureError {
    /// Platform not supported by scap (e.g. missing OS API).
    UnsupportedPlatform(String),
    /// OS did not grant screen-recording permission.
    NoPermission,
    /// No such source id (see [`crate::sources::list_sources`]).
    UnknownSource(String),
    /// scap failed to build or run the capturer.
    CaptureFailed(String),
    /// The ffmpeg encoder subprocess failed.
    EncoderFailed(String),
    /// A recording is already in progress on this handle.
    AlreadyRecording,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform(s) => write!(f, "platform not supported: {s}"),
            Self::NoPermission => write!(f, "screen recording permission not granted"),
            Self::UnknownSource(id) => write!(f, "unknown source: {id}"),
            Self::CaptureFailed(s) => write!(f, "capture failed: {s}"),
            Self::EncoderFailed(s) => write!(f, "encoder failed: {s}"),
            Self::AlreadyRecording => write!(f, "already recording"),
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

struct WorkerOutcome {
    frames: u64,
    error: Option<String>,
}

/// An active recording session. Owns the session clock.
pub struct Session {
    id: String,
    clock: SessionClock,
    output_path: PathBuf,
    fps: u32,
    width: u32,
    height: u32,
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<WorkerOutcome>>,
    encoder: Option<Child>,
}

impl Session {
    /// Start recording `source_id` (see [`crate::sources::list_sources`])
    /// at `fps`, encoding to `output_path` (MP4).
    ///
    /// The session clock starts now; share [`Session::clock`] with input
    /// telemetry before the first event is stamped.
    pub fn start(source_id: &str, fps: u32, output_path: &Path) -> Result<Self, CaptureError> {
        if !scap::is_supported() {
            return Err(CaptureError::UnsupportedPlatform(
                "scap reports this platform is not supported".into(),
            ));
        }
        if !scap::has_permission() {
            return Err(CaptureError::NoPermission);
        }
        let target =
            resolve_target(source_id).ok_or_else(|| CaptureError::UnknownSource(source_id.into()))?;
        // Linux portal flow: target None -> the OS picker appears at capture start.
        #[allow(unused_mut)]
        let mut scap_target: Option<scap::Target> = match target {
            ResolvedTarget::Scap(t) => Some(t),
            #[cfg(target_os = "linux")]
            ResolvedTarget::PortalPicker => None,
        };

        let options = scap::capturer::Options {
            fps,
            show_cursor: true,
            show_highlight: false,
            target: scap_target.take(),
            output_type: scap::frame::FrameType::BGRAFrame,
            ..Default::default()
        };
        // scap panics (instead of returning Err) when the platform backend
        // cannot even initialize — e.g. no D-Bus/portal session on Linux.
        // Catch it here so a Tauri command fails cleanly instead of
        // crashing the backend thread. The panic hook is silenced during
        // the build so users see our error, not scap's internals.
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let build_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scap::capturer::Capturer::build(options)
        }));
        std::panic::set_hook(prev_hook);
        let mut capturer = match build_result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                return Err(CaptureError::CaptureFailed(format!("{e:?}")));
            }
            Err(_) => {
                return Err(CaptureError::CaptureFailed(
                    "scap panicked during setup (no D-Bus/portal session?)".into(),
                ));
            }
        };
        let [w, h] = capturer.get_output_frame_size();

        let mut encoder = spawn_encoder(output_path, w, h, fps).map_err(CaptureError::EncoderFailed)?;
        let stdin = encoder
            .stdin
            .take()
            .ok_or_else(|| CaptureError::EncoderFailed("could not open encoder stdin".into()))?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag = stop_flag.clone();
        let clock = SessionClock::new();
        let id = format!("sess-{}", (clock.t0_wall() * 1000.0) as u64);

        let worker = thread::Builder::new()
            .name("flickcut-capture".into())
            .spawn(move || run_worker(&mut capturer, stdin, flag))
            .map_err(|e| CaptureError::CaptureFailed(format!("spawn worker: {e}")))?;

        Ok(Self {
            id,
            clock,
            output_path: output_path.to_path_buf(),
            fps,
            width: w,
            height: h,
            stop_flag,
            worker: Some(worker),
            encoder: Some(encoder),
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
    /// Always returns (never hangs): the worker is joined with a timeout and
    /// the encoder is killed as a last resort.
    pub fn stop(mut self) -> Result<RecordingSummary, CaptureError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        let outcome = match self.worker.take() {
            Some(h) => {
                // Cooperative stop first; the worker exits between frames.
                let (tx, rx) = std::sync::mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(h.join());
                });
                match rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(Ok(o)) => o,
                    Ok(Err(_)) => WorkerOutcome {
                        frames: 0,
                        error: Some("capture thread panicked".into()),
                    },
                    Err(_) => {
                        // Worker stuck (e.g. blocked in recv with a frozen
                        // source). Kill the encoder so nothing hangs; the
                        // orphan thread exits on the next frame.
                        if let Some(mut enc) = self.encoder.take() {
                            let _ = enc.kill();
                            let _ = enc.wait();
                        }
                        WorkerOutcome {
                            frames: 0,
                            error: Some("capture thread did not stop in time".into()),
                        }
                    }
                }
            }
            None => WorkerOutcome {
                frames: 0,
                error: Some("no worker thread".into()),
            },
        };

        // Normal path: close stdin (EOF) and wait for the encoder to finish
        // writing the MP4.
        if let Some(mut enc) = self.encoder.take() {
            // stdin was moved into the worker and dropped there on exit.
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

        let duration_sec = self.clock.elapsed_sec();
        Ok(RecordingSummary {
            session_id: self.id.clone(),
            video_path: self.output_path.clone(),
            duration_sec,
            frames_captured: outcome.frames,
            frames_written: outcome.frames,
            frames_dropped: 0,
            width: self.width,
            height: self.height,
            fps_target: self.fps,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Never leave a session recording on drop: best-effort stop.
        self.stop_flag.store(true, Ordering::SeqCst);
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

/// The worker: pull frames from scap, stamp them, feed the encoder.
fn run_worker(
    capturer: &mut scap::capturer::Capturer,
    mut stdin: ChildStdin,
    stop: Arc<AtomicBool>,
) -> WorkerOutcome {
    capturer.start_capture();
    let mut frames: u64 = 0;
    let mut error: Option<String> = None;

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match capturer.get_next_frame() {
            Ok(scap::frame::Frame::BGRA(frame)) => {
                if stdin.write_all(&frame.data).is_err() {
                    error = Some("encoder stdin closed".into());
                    break;
                }
                frames += 1;
            }
            Ok(_) => {
                // Non-BGRA frame (shouldn't happen; we requested BGRA).
                continue;
            }
            Err(_) => break, // channel disconnected
        }
    }

    capturer.stop_capture();
    drop(stdin); // EOF -> ffmpeg finalizes the file
    WorkerOutcome { frames, error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_source_is_rejected() {
        let r = Session::start("screen:999999", 60, Path::new("/tmp/nope.mp4"));
        assert!(matches!(r, Err(CaptureError::UnknownSource(_))));
    }

    #[test]
    fn malformed_source_is_rejected() {
        let r = Session::start("definitely-not-a-source", 60, Path::new("/tmp/nope.mp4"));
        assert!(matches!(r, Err(CaptureError::UnknownSource(_))));
    }
}
