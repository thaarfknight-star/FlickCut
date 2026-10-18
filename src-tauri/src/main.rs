// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

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

fn main() {
    tauri::Builder::default()
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            input_start,
            input_stop,
            input_trail,
            input_clear,
            analyze_moments,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlickCut");
}
