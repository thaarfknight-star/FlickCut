// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Windows backend: Windows Graphics Capture via `windows-capture` 2.x.
//!
//! Frames arrive on the capture thread through [`Handler::on_frame_arrived`]
//! and are forwarded (BGRA, unpadded) into the recorder's channel.
//!
//! NOTE: this module is `cfg(windows)`-gated and cannot be compile-checked on
//! other platforms; it was written against the `windows-capture` 2.0.1
//! sources and needs verification on native Windows CI.

use crate::backends::Backend;
use crate::frame::Frame as FcFrame;
use crate::recorder::CaptureError;
use crate::sources::{SourceList, SourceRef};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::time::Duration;
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler, InternalCaptureControl};
use windows_capture::frame::Frame;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

/// Channel sender passed through capture [`Settings`] flags.
struct Handler {
    tx: Sender<FcFrame>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Sender<FcFrame>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { tx: ctx.flags })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let mut buffer = frame.buffer()?;
        let (w, h) = (buffer.width(), buffer.height());
        let mut scratch = Vec::new();
        let pixels = buffer.as_nopadding_buffer(&mut scratch).to_vec();
        // Best-effort: if the recorder is slow, drop instead of blocking the
        // OS capture thread.
        let _ = self.tx.try_send(FcFrame {
            width: w,
            height: h,
            data: pixels,
        });
        Ok(())
    }
}

pub struct WindowsBackend {
    control: Option<CaptureControl<Handler, <Handler as GraphicsCaptureApiHandler>::Error>>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self { control: None }
    }
}

impl Backend for WindowsBackend {
    fn list_sources(&mut self) -> Result<SourceList, CaptureError> {
        let monitors = Monitor::enumerate().unwrap_or_default();
        let windows = Window::enumerate().unwrap_or_default();
        Ok(SourceList {
            screens: monitors
                .iter()
                .enumerate()
                .map(|(i, m)| SourceRef {
                    id: format!("screen:{i}"),
                    name: m.name().unwrap_or_else(|_| format!("Screen {i}")),
                })
                .collect(),
            windows: windows
                .iter()
                .enumerate()
                .map(|(i, w)| SourceRef {
                    id: format!("window:{i}"),
                    name: w.title().unwrap_or_else(|_| format!("Window {i}")),
                })
                .collect(),
        })
    }

    fn start(&mut self, source_id: &str, fps: u32) -> Result<Receiver<FcFrame>, CaptureError> {
        self.stop();
        let (kind, idx) = source_id
            .split_once(':')
            .ok_or_else(|| CaptureError::UnknownSource(source_id.into()))?;
        let idx: usize = idx
            .parse()
            .map_err(|_| CaptureError::UnknownSource(source_id.into()))?;

        let (tx, rx) = bounded::<FcFrame>(8);
        let interval = Duration::from_secs_f64(1.0 / fps.max(1) as f64);

        // NB: two concrete item types → two start calls (Settings is generic).
        let control = match kind {
            "screen" => {
                let monitor = Monitor::from_index(idx)
                    .map_err(|_| CaptureError::UnknownSource(source_id.into()))?;
                let settings = Settings::new(
                    monitor,
                    CursorCaptureSettings::WithCursor,
                    DrawBorderSettings::WithoutBorder,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Custom(interval),
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    tx,
                );
                Handler::start_free_threaded(settings)
            }
            "window" => {
                let window = Window::enumerate()
                    .unwrap_or_default()
                    .into_iter()
                    .nth(idx)
                    .ok_or_else(|| CaptureError::UnknownSource(source_id.into()))?;
                let settings = Settings::new(
                    window,
                    CursorCaptureSettings::WithCursor,
                    DrawBorderSettings::WithoutBorder,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Custom(interval),
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    tx,
                );
                Handler::start_free_threaded(settings)
            }
            _ => return Err(CaptureError::UnknownSource(source_id.into())),
        }
        .map_err(|e| CaptureError::CaptureFailed(format!("windows-capture start: {e:?}")))?;

        self.control = Some(control);
        Ok(rx)
    }

    fn stop(&mut self) {
        if let Some(control) = self.control.take() {
            let _ = control.stop();
        }
    }
}
