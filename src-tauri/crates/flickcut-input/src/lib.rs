//! `flickcut-input`: global mouse/keyboard/gamepad input telemetry.
//!
//! - Continuous cursor-position trail at ~30 Hz (the core Recordly signal)
//! - Mouse click positions (press-edge) and keyboard press events via `rdev`
//! - Gamepad buttons + triggers via `gilrs`, with hotplug
//!
//! All events are timestamped in milliseconds since a **shared session
//! clock** (`std::time::Instant`) supplied by the caller — the same epoch
//! the video recorder uses, so input events line up with video frames.
//!
//! In the Tauri app, wire it in `.setup()`:
//!
//! ```ignore
//! use tauri::Manager;
//! use std::sync::{Arc, Mutex};
//!
//! tauri::Builder::default()
//!     .setup(|app| {
//!         let handle = app.handle().clone();
//!         let on_event = Arc::new(move |ev: flickcut_input::InputEvent| {
//!             handle.emit("flickcut://input-event", ev).ok();
//!         });
//!         let telemetry = flickcut_input::InputTelemetry::new(Some(on_event), 30.0);
//!         app.manage(Arc::new(Mutex::new(telemetry)));
//!         Ok(())
//!     })
//! ```

use device_query::{DeviceQuery, DeviceState};
use gilrs::{Button, EventType as GilEventType, Gilrs};
use rdev::{listen, Button as RButton, EventType as REventType};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Event model
// ---------------------------------------------------------------------------

/// One input event on the shared session timeline.
#[derive(Clone, Debug, Serialize)]
pub struct InputEvent {
    /// Milliseconds since the shared session epoch.
    pub t_ms: f64,
    /// "cursor" | "mouse" | "keyboard" | "gamepad"
    pub source: &'static str,
    /// "move" | "click_left" | "key_down" | "btn_down" | ...
    pub action: String,
    /// Extra fields (x, y, key name, button id, fire flag, ...).
    pub detail: HashMap<String, String>,
}

impl InputEvent {
    fn new(
        epoch: Instant,
        source: &'static str,
        action: impl Into<String>,
        detail: HashMap<String, String>,
    ) -> Self {
        Self {
            t_ms: epoch.elapsed().as_secs_f64() * 1000.0,
            source,
            action: action.into(),
            detail,
        }
    }
}

/// One cursor-trail sample: timestamp + raw pixel position.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CursorPoint {
    pub t_ms: f64,
    pub x: i32,
    pub y: i32,
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

/// Callback type for live discrete events.
pub type EventCallback = Arc<dyn Fn(InputEvent) + Send + Sync>;

/// Background capture of mouse/keyboard/gamepad input.
///
/// Thread-safe. All timestamps use the `epoch` passed to
/// [`InputTelemetry::start`].
pub struct InputTelemetry {
    epoch: Option<Instant>,
    running: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<InputEvent>>>,
    trail: Arc<Mutex<Vec<CursorPoint>>>,
    on_event: Option<EventCallback>,
    cursor_hz: f64,
}

impl InputTelemetry {
    /// Create a stopped telemetry instance.
    ///
    /// `on_event` fires for every discrete event (clicks, keys, gamepad)
    /// — not for the 30 Hz cursor samples (poll those via [`Self::trail`]
    /// to avoid IPC spam).
    pub fn new(on_event: Option<EventCallback>, cursor_hz: f64) -> Self {
        Self {
            epoch: None,
            running: Arc::new(AtomicBool::new(false)),
            events: Arc::new(Mutex::new(Vec::new())),
            trail: Arc::new(Mutex::new(Vec::new())),
            on_event,
            cursor_hz: cursor_hz.max(1.0),
        }
    }

    /// Begin capture on the shared session `epoch`; returns the epoch used.
    ///
    /// A second `start` without `stop` is a no-op returning the old epoch.
    pub fn start(&mut self, epoch: Instant) -> Instant {
        if self.running.load(Ordering::Relaxed) {
            return self.epoch.unwrap();
        }
        self.epoch = Some(epoch);
        self.running.store(true, Ordering::Relaxed);

        // Cursor trail thread.
        {
            let running = self.running.clone();
            let trail = self.trail.clone();
            let interval = Duration::from_secs_f64(1.0 / self.cursor_hz);
            thread::Builder::new()
                .name("flickcut-cursor".into())
                .spawn(move || cursor_trail(epoch, running, trail, interval))
                .expect("spawn cursor thread");
        }
        // Global mouse/keyboard hooks (rdev blocks; own thread).
        {
            let events = self.events.clone();
            let on_event = self.on_event.clone();
            thread::Builder::new()
                .name("flickcut-hooks".into())
                .spawn(move || global_hooks(epoch, events, on_event))
                .expect("spawn hooks thread");
        }
        // Gamepad thread.
        {
            let running = self.running.clone();
            let events = self.events.clone();
            let on_event = self.on_event.clone();
            thread::Builder::new()
                .name("flickcut-gamepad".into())
                .spawn(move || gamepad_loop(epoch, running, events, on_event))
                .expect("spawn gamepad thread");
        }
        epoch
    }

    /// Signal all capture threads to stop (they exit on their next poll).
    ///
    /// Note: the rdev hook thread blocks inside `listen()` and only exits
    /// with the process; the pollers stop promptly.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Chronological snapshot of discrete events.
    pub fn events(&self) -> Vec<InputEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Chronological snapshot of the cursor trail.
    pub fn trail(&self) -> Vec<CursorPoint> {
        self.trail.lock().unwrap().clone()
    }

    /// Clear events and trail (keeps running state and epoch).
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
        self.trail.lock().unwrap().clear();
    }

    /// The session epoch, if started.
    pub fn epoch(&self) -> Option<Instant> {
        self.epoch
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    fn push_test_event(
        &self,
        source: &'static str,
        action: impl Into<String>,
        detail: HashMap<String, String>,
    ) {
        let epoch = self.epoch.expect("start() before pushing");
        let ev = InputEvent::new(epoch, source, action, detail);
        push_event(&self.events, &self.on_event, ev);
    }
}

fn push_event(
    events: &Arc<Mutex<Vec<InputEvent>>>,
    on_event: &Option<EventCallback>,
    ev: InputEvent,
) {
    events.lock().unwrap().push(ev.clone());
    if let Some(cb) = on_event {
        cb(ev);
    }
}

// ---------------------------------------------------------------------------
// Producers
// ---------------------------------------------------------------------------

/// Poll cursor position; append `(t_ms, x, y)` to the trail.
fn cursor_trail(
    epoch: Instant,
    running: Arc<AtomicBool>,
    trail: Arc<Mutex<Vec<CursorPoint>>>,
    interval: Duration,
) {
    let device = DeviceState::new();
    while running.load(Ordering::Relaxed) {
        let (x, y) = device.get_mouse().coords;
        trail.lock().unwrap().push(CursorPoint {
            t_ms: epoch.elapsed().as_secs_f64() * 1000.0,
            x,
            y,
        });
        thread::sleep(interval);
    }
}

/// Global mouse/keyboard hooks via rdev (press-edge only).
fn global_hooks(
    epoch: Instant,
    events: Arc<Mutex<Vec<InputEvent>>>,
    on_event: Option<EventCallback>,
) {
    if let Err(e) = listen(move |ev| {
        let (source, action, detail): (&'static str, String, HashMap<String, String>) =
            match ev.event_type {
                REventType::ButtonPress(b) => {
                    let name = match b {
                        RButton::Left => "left",
                        RButton::Right => "right",
                        RButton::Middle => "middle",
                        _ => "other",
                    };
                    ("mouse", format!("click_{}", name), HashMap::new())
                }
                REventType::KeyPress(k) => {
                    let mut d = HashMap::new();
                    d.insert("key".to_string(), format!("{:?}", k));
                    ("keyboard", "key_down".to_string(), d)
                }
                _ => return, // press-edge only; moves come from the poller
            };
        push_event(&events, &on_event, InputEvent::new(epoch, source, action, detail));
    }) {
        eprintln!("rdev listen failed: {:?}", e);
    }
}

/// Gamepad buttons/triggers via gilrs (hotplug handled internally).
fn gamepad_loop(
    epoch: Instant,
    running: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<InputEvent>>>,
    on_event: Option<EventCallback>,
) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gilrs init failed: {}", e);
            return;
        }
    };
    let fire = [Button::RightTrigger, Button::RightTrigger2];
    while running.load(Ordering::Relaxed) {
        while let Some(ev) = gilrs.next_event() {
            let (action, mut detail) = match ev.event {
                GilEventType::ButtonPressed(b, _) => {
                    let mut d = HashMap::new();
                    d.insert("button".to_string(), format!("{:?}", b));
                    d.insert("fire".to_string(), fire.contains(&b).to_string());
                    ("btn_down".to_string(), d)
                }
                GilEventType::Connected => ("connected".to_string(), HashMap::new()),
                GilEventType::Disconnected => {
                    ("disconnected".to_string(), HashMap::new())
                }
                _ => continue,
            };
            detail.insert("pad".to_string(), format!("{:?}", ev.id));
            push_event(
                &events,
                &on_event,
                InputEvent::new(epoch, "gamepad", action, detail),
            );
        }
        thread::sleep(Duration::from_millis(8));
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn timestamps_use_shared_epoch() {
        let epoch = Instant::now() - Duration::from_secs(5);
        let ev = InputEvent::new(epoch, "mouse", "click_left", HashMap::new());
        assert!(
            (4950.0..=5150.0).contains(&ev.t_ms),
            "t_ms={} not ~5000",
            ev.t_ms
        );
    }

    #[test]
    fn events_stay_chronological() {
        let epoch = Instant::now();
        let events = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..50 {
            events
                .lock()
                .unwrap()
                .push(InputEvent::new(epoch, "cursor", "move", HashMap::new()));
        }
        let guard = events.lock().unwrap();
        let ts: Vec<f64> = guard.iter().map(|e| e.t_ms).collect();
        let mut sorted = ts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(ts, sorted, "events must be chronological");
    }

    #[test]
    fn trail_points_chronological() {
        let epoch = Instant::now();
        let pts: Vec<CursorPoint> = (0..20)
            .map(|i| CursorPoint {
                t_ms: epoch.elapsed().as_secs_f64() * 1000.0,
                x: 100 + i,
                y: 200,
            })
            .collect();
        assert_eq!(pts.len(), 20);
        let ts: Vec<f64> = pts.iter().map(|p| p.t_ms).collect();
        let mut sorted = ts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(ts, sorted);
        assert_eq!((pts[0].x, pts[19].x), (100, 119));
    }

    #[test]
    fn shared_clock_sync_across_sources() {
        let epoch = Instant::now() - Duration::from_millis(2500);
        let a = InputEvent::new(epoch, "mouse", "click_left", HashMap::new());
        thread::sleep(Duration::from_millis(50));
        let b = InputEvent::new(epoch, "gamepad", "btn_down", HashMap::new());
        assert!(b.t_ms > a.t_ms);
        assert!((2500.0..=2700.0).contains(&a.t_ms), "a.t_ms={}", a.t_ms);
        assert!(
            (b.t_ms - a.t_ms - 50.0).abs() < 40.0,
            "delta not ~50ms: {}",
            b.t_ms - a.t_ms
        );
    }

    #[test]
    fn telemetry_start_stop_clear() {
        let mut t = InputTelemetry::new(None, 30.0);
        assert!(!t.running());
        let epoch = Instant::now();
        assert_eq!(t.start(epoch), epoch);
        assert!(t.running());
        assert_eq!(t.start(Instant::now()), epoch); // no-op
        assert_eq!(t.epoch(), Some(epoch));
        t.push_test_event("mouse", "click_left", HashMap::new());
        assert_eq!(t.events().len(), 1);
        t.stop();
        assert!(!t.running());
        t.clear();
        assert!(t.events().is_empty());
        assert!(t.trail().is_empty());
    }

    #[test]
    fn on_event_callback_fires() {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let cb: EventCallback = Arc::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        });
        let mut t = InputTelemetry::new(Some(cb), 30.0);
        t.start(Instant::now());
        t.push_test_event("keyboard", "key_down", HashMap::new());
        t.stop();
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn event_serializes_for_tauri_emit() {
        // `app.emit("flickcut://input-event", ev)` requires Serialize.
        let epoch = Instant::now();
        let ev = InputEvent::new(epoch, "mouse", "click_left", HashMap::new());
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(json.contains("\"t_ms\""));
        assert!(json.contains("click_left"));
    }
}
