// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! macOS backend: ScreenCaptureKit via `screencapturekit` 11.x.
//!
//! Frames arrive through a closure output handler and are forwarded (BGRA,
//! unpadded) into the recorder's channel.
//!
//! NOTE: this module is `cfg(target_os = "macos")`-gated and cannot be
//! compile-checked on other platforms; it was written against the
//! `screencapturekit` 11.0.0 sources and needs verification on native macOS
//! CI. Screen Recording permission is required — denial surfaces as a
//! [`CaptureError::NoPermission`]-style failure for the UI to guide on.

use crate::backends::Backend;
use crate::frame::Frame as FcFrame;
use crate::recorder::CaptureError;
use crate::sources::{SourceList, SourceRef};
use crossbeam_channel::{bounded, Receiver, Sender};
use screencapturekit::cm::sample_buffer::CMSampleBufferExt;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;

/// Extract tight BGRA bytes from a sample buffer.
fn extract_bgra(sample: &CMSampleBuffer) -> Option<FcFrame> {
    let pb = sample.pixel_buffer()?;
    let _guard = pb.lock(CVPixelBufferLockFlags::READ_ONLY).ok()?;
    let (w, h) = (pb.width(), pb.height());
    let stride = pb.bytes_per_row();
    let base = pb.base_address();
    if base.is_null() || w == 0 || h == 0 {
        return None;
    }
    let row_bytes = w * 4;
    let mut out = vec![0u8; h * row_bytes];
    // SAFETY: base is valid for h*stride bytes while the read lock is held.
    unsafe {
        for y in 0..h {
            let src = std::slice::from_raw_parts(base.add(y * stride), row_bytes);
            out[y * row_bytes..(y + 1) * row_bytes].copy_from_slice(src);
        }
    }
    Some(FcFrame {
        width: w as u32,
        height: h as u32,
        data: out,
    })
}

pub struct MacosBackend {
    stream: Option<SCStream>,
}

impl MacosBackend {
    pub fn new() -> Self {
        Self { stream: None }
    }
}

impl Backend for MacosBackend {
    fn list_sources(&mut self) -> Result<SourceList, CaptureError> {
        let content =
            SCShareableContent::get().map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;
        Ok(SourceList {
            screens: content
                .displays()
                .iter()
                .enumerate()
                .map(|(i, d)| SourceRef {
                    id: format!("screen:{i}"),
                    name: format!("Display {} ({}×{})", d.display_id(), d.width(), d.height()),
                })
                .collect(),
            windows: content
                .windows()
                .iter()
                .enumerate()
                .map(|(i, w)| SourceRef {
                    id: format!("window:{i}"),
                    name: w.title().unwrap_or_else(|| format!("Window {i}")),
                })
                .collect(),
        })
    }

    fn start(&mut self, source_id: &str, _fps: u32) -> Result<Receiver<FcFrame>, CaptureError> {
        self.stop();
        let (kind, idx) = source_id
            .split_once(':')
            .ok_or_else(|| CaptureError::UnknownSource(source_id.into()))?;
        let idx: usize = idx
            .parse()
            .map_err(|_| CaptureError::UnknownSource(source_id.into()))?;

        let content =
            SCShareableContent::get().map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;

        // Build the content filter + stream configuration for the target.
        let (filter, (w, h)) = match kind {
            "screen" => {
                let d = content
                    .displays()
                    .get(idx)
                    .ok_or_else(|| CaptureError::UnknownSource(source_id.into()))?;
                let filter = SCContentFilter::create()
                    .with_display(d)
                    .with_excluding_windows(&[])
                    .build()
                    .map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;
                (filter, (d.width(), d.height()))
            }
            "window" => {
                let win = content
                    .windows()
                    .get(idx)
                    .ok_or_else(|| CaptureError::UnknownSource(source_id.into()))?;
                let filter = SCContentFilter::create()
                    .with_window(win)
                    .build()
                    .map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;
                // Window size: fall back to the primary display size.
                let (w, h) = content
                    .displays()
                    .first()
                    .map(|d| (d.width(), d.height()))
                    .unwrap_or((1920, 1080));
                (filter, (w, h))
            }
            _ => return Err(CaptureError::UnknownSource(source_id.into())),
        };

        let config = SCStreamConfiguration::new()
            .with_width(w)
            .with_height(h)
            .with_pixel_format(PixelFormat::BGRA);

        let mut stream =
            SCStream::new(&filter, &config).map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;

        let (tx, rx) = bounded::<FcFrame>(8);
        let tx_handler: Sender<FcFrame> = tx.clone();
        stream
            .add_output_handler(
                move |sample: CMSampleBuffer, _of_type: SCStreamOutputType| {
                    if let Some(frame) = extract_bgra(&sample) {
                        let _ = tx_handler.try_send(frame);
                    }
                },
                SCStreamOutputType::Screen,
            )
            .map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;

        stream
            .start_capture()
            .map_err(|e| CaptureError::CaptureFailed(format!("{e:?}")))?;

        self.stream = Some(stream);
        Ok(rx)
    }

    fn stop(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.stop_capture();
        }
    }
}

