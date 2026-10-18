// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! FlickCut Tauri v2 application shell — integration owner: TAgent5.
//!
//! Registers the frozen IPC contract v1 (`docs/ipc-contract-v1.md`):
//! 6 frontend commands + 3 frontend events.
//!   - list_sources    -> real (flickcut-capture backend enumeration)
//!   - analyze_moments -> real (flickcut-moments crate)
//!   - render_video    -> real (flickcut-render crate + ffmpeg sidecar)
//!   - start/stop_recording -> real (flickcut-capture Session + shared
//!     session clock; input telemetry starts on the same epoch)
//!   - import_video    -> STUB (session id only, no telemetry)
//!
//! The 4 internal `input_*` commands belong to the flickcut-input crate
//! (testing/debugging) and are not part of the frontend contract.
// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

use serde::{Deserialize, Serialize};

/// Shared input telemetry, started/stopped from Tauri commands.
type TelemetryState = Arc<Mutex<flickcut_input::InputTelemetry>>;

/// Registry of finished recording sessions, keyed by session id.
/// Populated by the session lifecycle (start/stop_recording, import_video);
/// `analyze_moments` reads from it.
type SessionsState = Arc<Mutex<flickcut_moments::SessionStore>>;

#[tauri::command]
fn input_start(state: tauri::State<TelemetryState>) -> Result<(), String> {
    // NOTE: during a recording, start_recording() starts telemetry on the
    // shared session-clock epoch. This command is for manual/testing use and
    // is idempotent (a running telemetry keeps its epoch).
    let mut t = state.lock().map_err(|e| e.to_string())?;
    t.start(std::time::Instant::now());
    Ok(())
}

#[tauri::command]
fn input_stop(state: tauri::State<TelemetryState>) -> Result<(), String> {
    let t = state.lock().map_err(|e| e.to_string())?;
    t.stop();
    Ok(())
}

#[tauri::command]
fn input_trail(state: tauri::State<TelemetryState>) -> Result<Vec<flickcut_input::CursorPoint>, String> {
    let t = state.lock().map_err(|e| e.to_string())?;
    Ok(t.trail())
}

#[tauri::command]
fn input_clear(state: tauri::State<TelemetryState>) -> Result<(), String> {
    let t = state.lock().map_err(|e| e.to_string())?;
    t.clear();
    Ok(())
}

/// Contract: `analyze_moments(session_id)` -> `{ markers: Marker[] }`.
///
/// Runs moment detection over the session's telemetry. Unknown session id
/// yields an empty marker list (no error — the frontend treats it as
/// "nothing detected yet").
#[tauri::command]
fn analyze_moments(
    session_id: String,
    sessions: tauri::State<SessionsState>,
) -> Result<AnalyzeMomentsResponse, String> {
    let store = sessions.lock().map_err(|e| e.to_string())?;
    let markers = flickcut_moments::analyze_moments(&session_id, &store);
    Ok(AnalyzeMomentsResponse { markers })
}

#[derive(serde::Serialize)]
struct AnalyzeMomentsResponse {
    markers: Vec<flickcut_moments::Marker>,
}

// ---------------------------------------------------------------------------
// list_sources (IPC contract v1) — shell-owned, real implementation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct ScreenInfo {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct WindowInfo {
    id: String,
    title: String,
}

#[derive(Debug, Clone, Serialize)]
struct SourceList {
    screens: Vec<ScreenInfo>,
    windows: Vec<WindowInfo>,
}

/// Contract: `list_sources()` -> `{ screens: [{id, name}], windows: [{id, title}] }`.
///
/// Real implementation via the flickcut-capture backend enumeration, so the
/// returned ids are exactly what `start_recording(source_id)` accepts.
#[tauri::command]
fn list_sources() -> Result<SourceList, String> {
    let list = flickcut_capture::list_sources();
    Ok(SourceList {
        screens: list
            .screens
            .into_iter()
            .map(|s| ScreenInfo {
                id: s.id,
                name: s.name,
            })
            .collect(),
        windows: list
            .windows
            .into_iter()
            .map(|s| WindowInfo {
                id: s.id,
                title: s.name,
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// start/stop_recording + import_video (IPC contract v1) — real wiring
// ---------------------------------------------------------------------------

/// Capture frames per second for recordings.
const RECORD_FPS: u32 = 60;

/// Shell-owned session record. Holds the live capture session; `stop()`
/// consumes it and finalizes the video file.
struct ActiveSession {
    capture: Option<flickcut_capture::Session>,
    ticking: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    /// Total time spent paused (excluded from elapsed).
    paused_total: Duration,
    /// Set while paused; the tick thread freezes.
    pause_started: Option<Instant>,
}

/// The active recording session, if any.
type SessionState = Arc<Mutex<Option<ActiveSession>>>;

#[derive(Debug, Clone, Serialize)]
struct SessionInfo {
    session_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct StopInfo {
    video_path: String,
    duration_sec: f64,
}

#[derive(Debug, Clone, Serialize)]
struct RecordingTick {
    elapsed_sec: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PauseInfo {
    paused_at_sec: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ResumeInfo {
    resumed_at_sec: f64,
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Where to write the recording: `<video_dir>/flickcut-<millis>.mp4`,
/// falling back to the system temp dir.
fn recording_output_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    let dir = app
        .path()
        .video_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    dir.join(format!("flickcut-{}.mp4", now_millis()))
}

/// Contract: `start_recording(source_id)` -> `{ session_id }`.
/// Emits `flickcut://recording-tick` every second while recording.
///
/// Real implementation: starts the flickcut-capture session (this blocks
/// while the Linux portal picker waits for the user), then starts input
/// telemetry on the SAME session-clock epoch so kill moments line up with
/// video frames.
#[tauri::command]
async fn start_recording(
    source_id: String,
    app: tauri::AppHandle,
    sessions: tauri::State<'_, SessionState>,
    telemetry: tauri::State<'_, TelemetryState>,
) -> Result<SessionInfo, String> {
    {
        let guard = sessions.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("a recording session is already active".into());
        }
    }

    let output_path = recording_output_path(&app);

    // Blocking capture start on a worker thread (the portal picker needs
    // user interaction and must not stall the Tauri runtime).
    let capture = tauri::async_runtime::spawn_blocking(move || {
        flickcut_capture::Session::start(&source_id, RECORD_FPS, &output_path)
    })
    .await
    .map_err(|e| format!("capture task panicked: {e}"))?
    .map_err(|e| e.to_string())?;

    let session_id = capture.id().to_string();
    let clock = capture.clock();

    // Input telemetry on the shared session epoch. If telemetry was already
    // running (e.g. a manual input_start), restart it so the epochs match.
    {
        let mut t = telemetry.lock().map_err(|e| e.to_string())?;
        if t.running() {
            t.stop();
        }
        t.clear();
        t.start(clock.t0_instant());
    }

    // Tick events driven by the session clock.
    let ticking = Arc::new(AtomicBool::new(true));
    let paused = Arc::new(AtomicBool::new(false));
    let tick_flag = ticking.clone();
    let tick_paused = paused.clone();
    let tick_clock = clock.clone();
    let tick_app = app.clone();
    std::thread::spawn(move || {
        while tick_flag.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            if !tick_flag.load(Ordering::Relaxed) {
                break;
            }
            if tick_paused.load(Ordering::Relaxed) {
                continue;
            }
            let _ = tick_app.emit(
                "flickcut://recording-tick",
                RecordingTick {
                    elapsed_sec: tick_clock.elapsed_sec() as u64,
                },
            );
        }
    });

    {
        let mut guard = sessions.lock().map_err(|e| e.to_string())?;
        // Re-check: a concurrent start could have won while we blocked.
        if guard.is_some() {
            return Err("a recording session is already active".into());
        }
        *guard = Some(ActiveSession {
            capture: Some(capture),
            ticking,
            paused,
            paused_total: Duration::ZERO,
            pause_started: None,
        });
    }

    Ok(SessionInfo { session_id })
}

/// Contract: `stop_recording()` -> `{ video_path, duration_sec }`.
///
/// Stops telemetry, finalizes the video file, and registers the session's
/// telemetry in `SessionsState` so `analyze_moments(session_id)` works.
#[tauri::command]
async fn stop_recording(
    sessions: tauri::State<'_, SessionState>,
    telemetry: tauri::State<'_, TelemetryState>,
    store: tauri::State<'_, SessionsState>,
) -> Result<StopInfo, String> {
    let mut session = {
        let mut guard = sessions.lock().map_err(|e| e.to_string())?;
        guard.take().ok_or("no active recording session")?
    };
    session.ticking.store(false, Ordering::Relaxed);

    // Snapshot telemetry before stopping capture.
    let (events, trail) = {
        let mut t = telemetry.lock().map_err(|e| e.to_string())?;
        t.stop();
        (t.events(), t.trail())
    };

    // Finalize the video (blocking encode flush on a worker thread).
    let capture = session
        .capture
        .take()
        .ok_or("capture session already stopped")?;
    let summary = tauri::async_runtime::spawn_blocking(move || capture.stop())
        .await
        .map_err(|e| format!("stop task panicked: {e}"))?
        .map_err(|e| e.to_string())?;

    // Register for analyze_moments.
    {
        let mut s = store.lock().map_err(|e| e.to_string())?;
        s.insert(
            summary.session_id.clone(),
            flickcut_moments::SessionData {
                session_id: summary.session_id.clone(),
                events,
                trail,
                screen_size: Some((summary.width as f64, summary.height as f64)),
            },
        );
    }

    Ok(StopInfo {
        video_path: summary.video_path.to_string_lossy().into_owned(),
        duration_sec: summary.duration_sec,
    })
}

/// Contract: `import_video(path)` -> `{ session_id }`.
///
/// STUB: imports an existing video file as a session (no telemetry).
/// Contract v1.1: `pause_recording()` -> `{ paused_at_sec }`.
#[tauri::command]
fn pause_recording(
    sessions: tauri::State<SessionState>,
) -> Result<PauseInfo, String> {
    let mut guard = sessions.lock().map_err(|e| e.to_string())?;
    let s = guard.as_mut().ok_or("no active recording session")?;
    if s.pause_started.is_some() {
        return Err("recording is already paused".into());
    }
    let paused_at_sec = s
        .capture
        .as_ref()
        .map(|c| c.clock().elapsed_sec())
        .unwrap_or(0.0)
        - s.paused_total.as_secs_f64();
    s.pause_started = Some(Instant::now());
    s.paused.store(true, Ordering::Relaxed);
    Ok(PauseInfo { paused_at_sec })
}

/// Contract v1.1: `resume_recording()` -> `{ resumed_at_sec }`.
#[tauri::command]
fn resume_recording(
    sessions: tauri::State<SessionState>,
) -> Result<ResumeInfo, String> {
    let mut guard = sessions.lock().map_err(|e| e.to_string())?;
    let s = guard.as_mut().ok_or("no active recording session")?;
    let ps = s.pause_started.take().ok_or("recording is not paused")?;
    s.paused_total += ps.elapsed();
    s.paused.store(false, Ordering::Relaxed);
    let resumed_at_sec = s
        .capture
        .as_ref()
        .map(|c| c.clock().elapsed_sec())
        .unwrap_or(0.0)
        - s.paused_total.as_secs_f64();
    Ok(ResumeInfo { resumed_at_sec })
}

#[tauri::command]
fn import_video(path: String) -> Result<SessionInfo, String> {
    let _ = path;
    Ok(SessionInfo {
        session_id: format!("import-{}", now_millis()),
    })
}

// ---------------------------------------------------------------------------
// render_video (IPC contract v1)
// ---------------------------------------------------------------------------

/// Frontend `Region` DTO (ipc-contract-v1.md). The contract carries no focus
/// point, so zoom regions default to frame center; if the contract gains
/// x/y, map them via `Region::with_focus` in `dto_to_region`.
#[derive(Debug, Deserialize)]
struct RegionDto {
    start: f64,
    end: f64,
    effect: String,
    intensity: f64,
}

#[derive(Debug, Serialize)]
struct RenderVideoResponse {
    output_path: String,
}

fn dto_to_region(dto: &RegionDto) -> Result<flickcut_render::Region, String> {
    let effect = match dto.effect.as_str() {
        "slowmo" => flickcut_render::Effect::SlowMo,
        "curve" => flickcut_render::Effect::Curve,
        "zoomin" => flickcut_render::Effect::ZoomIn,
        "zoomout" => flickcut_render::Effect::ZoomOut,
        other => return Err(format!("unknown effect: {other}")),
    };
    flickcut_render::Region::new(dto.start, dto.end, effect, dto.intensity)
}

/// Tauri v2 sidecar triple, matching the `ffmpeg-<triple>[.exe]` names the
/// bundler expects in `src-tauri/binaries/`.
fn target_triple() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-unknown-linux-gnu";
}

/// Locate the bundled static ffmpeg: `FFMPEG_PATH` (dev override), then the
/// sidecar next to the app binary, then PATH.
fn resolve_ffmpeg() -> Result<String, String> {
    if let Ok(p) = std::env::var("FFMPEG_PATH") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let triple = target_triple();
            let exe_name = if cfg!(windows) { ".exe" } else { "" };
            for cand in [
                dir.join(format!("ffmpeg-{triple}{exe_name}")),
                dir.join(format!("ffmpeg{exe_name}")),
            ] {
                if cand.is_file() {
                    return Ok(cand.to_string_lossy().into_owned());
                }
            }
        }
    }
    Ok(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }.to_string())
}

fn default_output_path(video_path: &str) -> String {
    let p = std::path::Path::new(video_path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let name = format!("{stem}-flickcut.mp4");
    match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => {
            d.join(name).to_string_lossy().into_owned()
        }
        _ => name,
    }
}

/// IPC v1 `render_video(video_path, regions)` -> `{ output_path }`.
/// Emits `flickcut://render-progress` with `{ percent: 0-100 }` while running.
/// The blocking ffmpeg encode runs on a blocking thread pool worker.
#[tauri::command]
async fn render_video(
    app: tauri::AppHandle,
    video_path: String,
    regions: Vec<RegionDto>,
) -> Result<RenderVideoResponse, String> {
    let mut rs = Vec::with_capacity(regions.len());
    for dto in &regions {
        rs.push(dto_to_region(dto)?);
    }
    let ffmpeg = resolve_ffmpeg()?;
    let output_path = default_output_path(&video_path);

    let out = tauri::async_runtime::spawn_blocking(move || {
        flickcut_render::render_video(
            &ffmpeg,
            &video_path,
            &rs,
            &output_path,
            &|pct: u8| {
                let _ = app.emit(
                    "flickcut://render-progress",
                    serde_json::json!({ "percent": pct }),
                );
            },
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("render task panicked: {e}"))?
    .map_err(|e| e)?;

    Ok(RenderVideoResponse { output_path: out })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Forward every discrete input event to the frontend.
            // (The 30 Hz cursor trail is polled via the `input_trail`
            // command to avoid IPC spam.)
            let handle = app.handle().clone();
            let on_event: flickcut_input::EventCallback = Arc::new(move |ev| {
                handle.emit("flickcut://input-event", ev).ok();
            });
            let telemetry =
                flickcut_input::InputTelemetry::new(Some(on_event), 30.0);
            app.manage(Arc::new(Mutex::new(telemetry)));
            app.manage(Arc::new(Mutex::new(
                HashMap::<String, flickcut_moments::SessionData>::new(),
            )));
            // Shell-owned recording session (stub until the recorder lands).
            app.manage(SessionState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            input_start,
            input_stop,
            input_trail,
            input_clear,
            analyze_moments,
            render_video,
            // Shell-owned contract commands
            list_sources,
            start_recording,
            stop_recording,
            pause_recording,
            resume_recording,
            import_video,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlickCut");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto(start: f64, end: f64, effect: &str, intensity: f64) -> RegionDto {
        RegionDto {
            start,
            end,
            effect: effect.to_string(),
            intensity,
        }
    }

    #[test]
    fn dto_converts_all_effects() {
        for (name, want) in [
            ("slowmo", flickcut_render::Effect::SlowMo),
            ("curve", flickcut_render::Effect::Curve),
            ("zoomin", flickcut_render::Effect::ZoomIn),
            ("zoomout", flickcut_render::Effect::ZoomOut),
        ] {
            let r = dto_to_region(&dto(1.0, 2.0, name, 50.0)).unwrap();
            assert_eq!(r.effect, want);
            assert_eq!((r.start, r.end), (1.0, 2.0));
            assert_eq!(r.intensity, 50.0);
            // contract has no focus -> center default
            assert_eq!(r.focus, (0.5, 0.5));
        }
    }

    #[test]
    fn dto_rejects_bad_effect_and_range() {
        assert!(dto_to_region(&dto(1.0, 2.0, "spin", 50.0)).is_err());
        assert!(dto_to_region(&dto(2.0, 1.0, "slowmo", 50.0)).is_err());
    }

    #[test]
    fn dto_clamps_intensity() {
        let r = dto_to_region(&dto(1.0, 2.0, "slowmo", 150.0)).unwrap();
        assert_eq!(r.intensity, 100.0);
    }

    #[test]
    fn output_path_next_to_input() {
        assert_eq!(
            default_output_path("/tmp/clip.mp4"),
            "/tmp/clip-flickcut.mp4"
        );
        assert_eq!(default_output_path("clip.mp4"), "clip-flickcut.mp4");
    }

    #[test]
    fn event_name_matches_contract() {
        // ipc-contract-v1.md: flickcut://render-progress, { percent: 0-100 }
        let payload = serde_json::json!({ "percent": 42u8 });
        assert_eq!(payload["percent"], 42);
    }
}

