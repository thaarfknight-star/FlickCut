// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! flickcut-capture: native screen capture for the FlickCut Tauri v2 backend.
//!
//! Platform backends (see [`backends`]):
//! - Windows: Windows Graphics Capture (`windows-capture`)
//! - macOS: ScreenCaptureKit (`screencapturekit`)
//! - Linux: XDG portal ScreenCast + PipeWire (`ashpd` + `pipewire`)
//!
//! Contract (see `docs/ipc-contract-v1.md`):
//! - [`sources::list_sources`] implements `list_sources()`
//! - [`recorder::Session`] implements `start_recording(source_id)` /
//!   `stop_recording()`; the session clock ([`clock::SessionClock`]) is owned
//!   here and shared with the input-telemetry module so kill moments line up
//!   with video frames.

pub mod backends;
pub mod clock;
pub mod frame;
pub mod recorder;
pub mod sources;

pub use backends::Backend;
pub use clock::{wall_now, SessionClock};
pub use frame::Frame;
pub use recorder::{CaptureError, RecordingSummary, Session};
pub use sources::{list_sources, SourceKind, SourceList, SourceRef};
