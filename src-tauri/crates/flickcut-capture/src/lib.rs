//! flickcut-capture: native screen capture for the FlickCut Tauri v2 backend.
//!
//! Contract (see `docs/ipc-contract-v1.md`):
//! - [`sources::list_sources`] implements `list_sources()`
//! - [`recorder::Session`] implements `start_recording(source_id)` /
//!   `stop_recording()`; the session clock ([`clock::SessionClock`]) is owned
//!   here and shared with the input-telemetry module (TAgent1) so kill
//!   moments line up with video frames.

pub mod clock;
pub mod recorder;
pub mod sources;

pub use clock::{wall_now, SessionClock};
pub use recorder::{CaptureError, RecordingSummary, Session};
pub use sources::{list_sources, SourceKind, SourceList, SourceRef};
