// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Platform capture backends behind one trait.
//!
//! - Windows: Windows Graphics Capture via `windows-capture`
//! - macOS: ScreenCaptureKit via `screencapturekit`
//! - Linux: XDG portal ScreenCast + PipeWire via `ashpd` + `pipewire`
//!
//! Every backend pushes [`crate::frame::Frame`] values (BGRA, no padding)
//! into a channel; the recorder drains it into the ffmpeg encoder.

use crate::frame::Frame;
use crate::recorder::CaptureError;
use crate::sources::SourceList;
use crossbeam_channel::Receiver;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

/// Native capture backend for the current platform.
pub trait Backend: Send {
    /// Enumerate capturable sources.
    fn list_sources(&mut self) -> Result<SourceList, CaptureError>;

    /// Start capturing `source_id` at `fps`, pushing [`Frame`]s into the
    /// returned channel. The channel closes when capture stops or fails.
    fn start(&mut self, source_id: &str, fps: u32) -> Result<Receiver<Frame>, CaptureError>;

    /// Stop an active capture (idempotent).
    fn stop(&mut self);
}

/// Build the backend for the current platform.
pub fn create_backend() -> Box<dyn Backend> {
    #[cfg(windows)]
    return Box::new(windows::WindowsBackend::new());
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacosBackend::new());
    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxBackend::new());
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    compile_error!("flickcut-capture: unsupported platform");
}
