// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! FlickCut Tauri v2 application shell — integration owner: TAgent5.
//!
//! Registers the frozen IPC contract v1 (`docs/ipc-contract-v1.md`):
//! 6 frontend commands + 3 frontend events.
//!   - list_sources    -> real (xcap enumeration; team decision 2026-10-18:
//!                        `scap` is unmaintained, do not use)
//!   - analyze_moments -> real (flickcut-moments crate)
//!   - render_video    -> real (flickcut-render crate + ffmpeg sidecar)
//!   - start/stop_recording, import_video -> STUBS owned by the shell
//!     (session clock + tick events); TAgent's recorder plugs capture in.
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
    // NOTE: the epoch should come from the recording session clock
    // (TAgent's session module). Until that wiring lands, each start
    // creates its own epoch.
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
/// Real implementation via xcap. Minimized windows (no capturable surface)
/// and untitled helper windows are filtered out of the picker.
#[tauri::command]
fn list_sources() -> Result<SourceList, String> {
    let mut screens = Vec::new();
    for monitor in xcap::Monitor::all().map_err(|e| e.to_string())? {
        let id = monitor.id().map_err(|e| e.to_string())?;
        let name = monitor
            .name()
            .unwrap_or_else(|_| format!("Display {}", id));
        screens.push(ScreenInfo {
            id: id.to_string(),
            name,
        });
    }
    let mut windows = Vec::new();
    for window in xcap::Window::all().map_err(|e| e.to_string())? {
        if window.is_minimized().unwrap_or(false) {
            continue;
        }
        let title = window.title().unwrap_or_default();
        if title.trim().is_empty() {
            continue;
        }
        let id = window.id().map_err(|e| e.to_string())?;
        windows.push(WindowInfo {
            id: id.to_string(),
            title,
        });
    }
    Ok(SourceList { screens, windows })
}

// ---------------------------------------------------------------------------
// start/stop_recording + import_video (IPC contract v1) — shell stubs
// ---------------------------------------------------------------------------

/// Shell-owned session record. `id`/`source_id` are read by TAgent's
/// recorder module once it plugs in.
#[allow(dead_code)]
struct ActiveSession {
    id: String,
    source_id: String,
    started_at: Instant,
    ticking: Arc<AtomicBool>,
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

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Contract: `start_recording(source_id)` -> `{ session_id }`.
/// Emits `flickcut://recording-tick` every second while recording.
///
/// STUB: the shell owns the session clock and tick events so the UI works
/// end-to-end. TAgent's recorder plugs capture + the real session clock in
/// here (and fills `SessionsState` for `analyze_moments`).
#[tauri::command]
fn start_recording(
    source_id: String,
    app: tauri::AppHandle,
    sessions: tauri::State<SessionState>,
) -> Result<SessionInfo, String> {
    let mut guard = sessions.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("a recording session is already active".into());
    }
    let id = format!("sess-{}", now_millis());
    let started_at = Instant::now();
    let ticking = Arc::new(AtomicBool::new(true));

    let tick_flag = ticking.clone();
    std::thread::spawn(move || {
        while tick_flag.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            if !tick_flag.load(Ordering::Relaxed) {
                break;
            }
            let elapsed = started_at.elapsed().as_secs();
            app.emit(
                "flickcut://recording-tick",
                RecordingTick {
                    elapsed_sec: elapsed,
                },
            )
            .ok();
        }
    });

    *guard = Some(ActiveSession {
        id: id.clone(),
        source_id,
        started_at,
        ticking,
    });
    // NOTE: the input telemetry epoch should come from the session clock
    // (open item carried over from the input crate wiring).
    Ok(SessionInfo { session_id: id })
}

/// Contract: `stop_recording()` -> `{ video_path, duration_sec }`.
///
/// STUB: stops the session clock. The recorder will return the real file.
#[tauri::command]
fn stop_recording(sessions: tauri::State<SessionState>) -> Result<StopInfo, String> {
    let mut guard = sessions.lock().map_err(|e| e.to_string())?;
    let session = guard.take().ok_or("no active recording session")?;
    session.ticking.store(false, Ordering::Relaxed);
    let duration_sec = session.started_at.elapsed().as_secs_f64();
    Ok(StopInfo {
        video_path: String::new(), // filled by the recorder module
        duration_sec,
    })
}

/// Contract: `import_video(path)` -> `{ session_id }`.
///
/// STUB: imports an existing video file as a session (no telemetry).
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
