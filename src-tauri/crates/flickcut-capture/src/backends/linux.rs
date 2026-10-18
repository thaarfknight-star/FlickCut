// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Linux backend: XDG Desktop Portal ScreenCast + PipeWire.
//!
//! Flow: portal session (ashpd) → user picks a source in the OS picker →
//! PipeWire node id + fd → pipewire stream → BGRA frames.
//!
//! Provenance note: the PipeWire stream setup (format pod, param/process
//! callbacks) follows the same approach as the pipewire-based capturers in
//! the ecosystem; the portal handshake uses `ashpd`.

use crate::backends::Backend;
use crate::frame::Frame;
use crate::recorder::CaptureError;
use crate::sources::{SourceList, SourceRef};
use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use crossbeam_channel::{bounded, Receiver, Sender};
use pipewire as pw;
use pw::context::Context;
use pw::main_loop::MainLoop;
use pw::properties::properties;
use pw::spa::param::format::{FormatProperties, MediaSubtype, MediaType};
use pw::spa::param::video::VideoFormat;
use pw::spa::utils::Direction;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Id of the synthetic portal source offered by [`list_sources`].
pub const PORTAL_SOURCE_ID: &str = "screen:portal";

struct WorkerState {
    quit: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub struct LinuxBackend {
    worker: Option<WorkerState>,
}

impl LinuxBackend {
    pub fn new() -> Self {
        Self { worker: None }
    }
}

impl Backend for LinuxBackend {
    fn list_sources(&mut self) -> Result<SourceList, CaptureError> {
        // The portal owns source enumeration (its picker lists screens and
        // windows), so we expose one synthetic entry.
        Ok(SourceList {
            screens: vec![SourceRef {
                id: PORTAL_SOURCE_ID.into(),
                name: "Choose source… (system picker)".into(),
            }],
            windows: vec![],
        })
    }

    fn start(&mut self, source_id: &str, fps: u32) -> Result<Receiver<Frame>, CaptureError> {
        if source_id != PORTAL_SOURCE_ID {
            return Err(CaptureError::UnknownSource(source_id.into()));
        }
        self.stop(); // idempotent: stop any previous capture

        let (tx, rx) = bounded::<Frame>(8);
        let quit = Arc::new(AtomicBool::new(false));
        let quit_flag = quit.clone();

        let handle = std::thread::Builder::new()
            .name("flickcut-pw".into())
            .spawn(move || {
                // The only Sender: if this thread exits (error/panic/quit),
                // the channel closes and the recorder sees it immediately.
                if let Err(e) = run_capture(fps, tx, quit_flag) {
                    eprintln!("flickcut-capture(linux): {e}");
                }
            })
            .map_err(|e| CaptureError::CaptureFailed(format!("spawn pipewire thread: {e}")))?;

        self.worker = Some(WorkerState { quit, handle });
        Ok(rx)
    }

    fn stop(&mut self) {
        if let Some(w) = self.worker.take() {
            w.quit.store(true, Ordering::SeqCst);
            let _ = w.handle.join();
        }
    }
}

/// Portal handshake: returns (pipewire fd, node id).
///
/// zbus requires a Tokio runtime; we run a minimal current-thread one just
/// for this handshake.
fn portal_setup() -> Result<(std::os::fd::OwnedFd, u32), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?;
    rt.block_on(async {
        let proxy = Screencast::new()
            .await
            .map_err(|e| format!("portal: {e}"))?;
        let session = proxy
            .create_session()
            .await
            .map_err(|e| format!("portal create_session: {e}"))?;
        proxy
            .select_sources(
                &session,
                CursorMode::Embedded,
                SourceType::Monitor | SourceType::Window,
                true,
                None,
                PersistMode::DoNot,
            )
            .await
            .map_err(|e| format!("portal select_sources: {e}"))?;
        // This pops the OS source picker; the user picks a screen/window.
        let streams = proxy
            .start(&session, None)
            .await
            .map_err(|e| format!("portal start: {e}"))?
            .response()
            .map_err(|e| format!("portal start response: {e}"))?;
        let stream = streams
            .streams()
            .first()
            .ok_or_else(|| "portal returned no streams (picker cancelled?)".to_string())?;
        let node_id = stream.pipe_wire_node_id();
        let fd = proxy
            .open_pipe_wire_remote(&session)
            .await
            .map_err(|e| format!("portal open_pipe_wire_remote: {e}"))?;
        Ok((fd, node_id))
    })
}

#[derive(Clone)]
struct ListenerData {
    tx: Sender<Frame>,
    format: pw::spa::param::video::VideoInfoRaw,
}

fn run_capture(fps: u32, tx: Sender<Frame>, quit: Arc<AtomicBool>) -> Result<(), String> {
    let (fd, node_id) = portal_setup()?;

    pw::init();
    let mainloop = MainLoop::new(None).map_err(|e| format!("pipewire mainloop: {e}"))?;
    let context = Context::new(&mainloop).map_err(|e| format!("pipewire context: {e}"))?;
    let core = context
        .connect_fd(fd, None)
        .map_err(|e| format!("pipewire connect_fd: {e}"))?;

    let user_data = ListenerData {
        tx,
        format: Default::default(),
    };

    let stream = pw::stream::Stream::new(
        &core,
        "flickcut",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(|e| format!("pipewire stream: {e}"))?;

    let _listener = stream
        .add_local_listener_with_user_data(user_data)
        .param_changed(|_stream, data, id, param| {
            use pw::spa::param::ParamType;
            let Some(param) = param else { return };
            if id != ParamType::Format.as_raw() {
                return;
            }
            let (media_type, media_subtype) =
                match pw::spa::param::format_utils::parse_format(param) {
                    Ok(v) => v,
                    Err(_) => return,
                };
            if media_type != MediaType::Video || media_subtype != MediaSubtype::Raw {
                return;
            }
            let _ = data.format.parse(param);
        })
        .process(|stream, data| {
            process_frame(stream, data);
        })
        .register()
        .map_err(|e| format!("pipewire listener: {e}"))?;

    let format_bytes = build_format_pod_bytes(fps).map_err(|e| format!("format pod: {e}"))?;
    let format_pod =
        pw::spa::pod::Pod::from_bytes(&format_bytes).ok_or("invalid format pod")?;
    let mut params = [format_pod];
    stream
        .connect(
            Direction::Input,
            Some(node_id),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| format!("pipewire connect: {e}"))?;

    let pw_loop = mainloop.loop_();
    while !quit.load(Ordering::SeqCst) {
        pw_loop.iterate(Duration::from_millis(50));
    }
    Ok(())
}

/// Build the EnumFormat pod bytes: raw video, BGRA preferred, size + fps ranges.
fn build_format_pod_bytes(fps: u32) -> Result<Vec<u8>, String> {
    use pw::spa::pod::serialize::PodSerializer;
    use pw::spa::pod::{object, property, Value};
    use pw::spa::utils::{Fraction, Rectangle, SpaTypes};

    let obj = object!(
        SpaTypes::ObjectParamFormat,
        pw::spa::param::ParamType::EnumFormat,
        property!(FormatProperties::MediaType, Id, MediaType::Video),
        property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRA,
            VideoFormat::BGRx
        ),
        property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            Rectangle { width: 128, height: 128 },
            Rectangle { width: 1, height: 1 },
            Rectangle { width: 8192, height: 8192 }
        ),
        property!(
            FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            Fraction { num: fps, denom: 1 },
            Fraction { num: 0, denom: 1 },
            Fraction { num: 1000, denom: 1 }
        )
    );
    let bytes = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
        .map_err(|e| format!("serialize format pod: {e}"))?
        .0
        .into_inner();
    Ok(bytes)
}

fn process_frame(stream: &pw::stream::StreamRef, data: &mut ListenerData) {
    use pw::spa::param::video::VideoInfoRaw;

    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };
    let format: VideoInfoRaw = data.format.clone();
    let size = format.size();
    let (w, h) = (size.width as usize, size.height as usize);
    if w == 0 || h == 0 {
        return;
    }

    let datas = buffer.datas_mut();
    let Some(pw_data) = datas.first_mut() else { return };
    let chunk = pw_data.chunk().clone();
    let stride = chunk.stride() as usize;
    let offset = chunk.offset() as usize;
    let Some(mapped) = pw_data.data() else { return };

    // Copy row by row (stride may include padding); normalize to tight BGRA.
    let row_bytes = w * 4;
    if mapped.len() < offset + (h - 1) * stride + row_bytes {
        return;
    }
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let src_start = offset + y * stride;
        out[y * row_bytes..(y + 1) * row_bytes]
            .copy_from_slice(&mapped[src_start..src_start + row_bytes]);
    }

    match format.format() {
        VideoFormat::BGRA => {
            let _ = data.tx.try_send(Frame { width: w as u32, height: h as u32, data: out });
        }
        VideoFormat::BGRx => {
            // BGRx → BGRA: force alpha to opaque.
            for px in out.chunks_exact_mut(4) {
                px[3] = 255;
            }
            let _ = data.tx.try_send(Frame { width: w as u32, height: h as u32, data: out });
        }
        _ => {
            // Format we didn't ask for; drop.
        }
    }
    // `buffer` drops here → re-queued to the stream.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_source_listed() {
        let mut b = LinuxBackend::new();
        let list = b.list_sources().unwrap();
        assert_eq!(list.screens.len(), 1);
        assert_eq!(list.screens[0].id, PORTAL_SOURCE_ID);
    }

    #[test]
    fn unknown_source_rejected() {
        let mut b = LinuxBackend::new();
        assert!(b.start("screen:0", 30).is_err());
    }

    #[test]
    fn format_pod_builds() {
        // Pure-SPA construction; must not panic and must parse back.
        let bytes = build_format_pod_bytes(60).expect("format pod");
        let pod = pw::spa::pod::Pod::from_bytes(&bytes).expect("pod ref");
        let (mt, mst) = pw::spa::param::format_utils::parse_format(pod).expect("parse back");
        assert_eq!(mt, MediaType::Video);
        assert_eq!(mst, MediaSubtype::Raw);
    }
}
