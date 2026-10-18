//! `flickcut-input`: global mouse/keyboard/gamepad input telemetry.
//!
//! - Continuous cursor-position trail at ~30 Hz (the core Recordly signal)
//! - Mouse click positions (press-edge) and keyboard press events via `rdev`
//! - Gamepad buttons + triggers via `gilrs`, with hotplug
//!
//! All events are timestamped from a **shared session clock**
//! ([`Clock`]) supplied by the caller — the same time base the video
//! recorder uses, so input events line up 1:1 with video frames.
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
//!
//! The recording session hands its `SessionClock` to
//! [`InputTelemetry::start_with_clock`]; implement [`Clock`] for it.

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
// Session clock abstraction
// ---------------------------------------------------------------------------

/// Shared session clock (implemented by `flickcut-capture::SessionClock`).
///
/// `stamp()` returns `(t_wall, t_mono)`: wall-clock epoch seconds and
/// monotonic seconds since session start. Input events use `t_mono` so
/// they line up 1:1 with video frame stamps; `t_wall` is kept for logs.
pub trait Clock: Send + Sync {
    fn stamp(&self) -> (f64, f64);
}

/// `Clock` adapter for a plain `Instant` epoch (used by `start()`).
#[derive(Clone)]
struct InstantClock {
    epoch: Instant,
    t0_wall: f64,
}

impl InstantClock {
    fn new(epoch: Instant) -> Self {
        Self {
            epoch,
            t0_wall: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
        }
    }
}

impl Clock for InstantClock {
    fn stamp(&self) -> (f64, f64) {
        let mono = self.epoch.elapsed().as_secs_f64();
        (self.t0_wall + mono, mono)
    }
}

// ---------------------------------------------------------------------------
// Event model
// ---------------------------------------------------------------------------

/// One input event on the shared session timeline.
#[derive(Clone, Debug, Serialize)]
pub struct InputEvent {
    /// Milliseconds since session start (monotonic, == video time base).
    pub t_ms: f64,
    /// Wall-clock epoch seconds (for logs/filenames).
    pub t_wall: f64,
    /// "cursor" | "mouse" | "keyboard" | "gamepad"
    pub source: &'static str,
    /// "move" | "click_left" | "key_down" | "btn_down" | ...
    pub action: String,
    /// Extra fields (x, y, key name, button id, fire flag, ...).
    pub detail: HashMap<String, String>,
}

impl InputEvent {
    fn new(
        clock: &Arc<dyn Clock>,
        source: &'static str,
        action: impl Into<String>,
        detail: HashMap<String, String>,
    ) -> Self {
        let (t_wall, t_mono) = clock.stamp();
        Self {
            t_ms: t_mono * 1000.0,
            t_wall,
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
/// Thread-safe. All timestamps come from the [`Clock`] passed to
/// [`InputTelemetry::start`] / [`InputTelemetry::start_with_clock`].
pub struct InputTelemetry {
    clock: Option<Arc<dyn Clock>>,
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
            clock: None,
            running: Arc::new(AtomicBool::new(false)),
            events: Arc::new(Mutex::new(Vec::new())),
            trail: Arc::new(Mutex::new(Vec::new())),
            on_event,
            cursor_hz: cursor_hz.max(1.0),
        }
    }

    /// Begin capture on a plain `Instant` epoch (kept for tests/standalone
    /// use). In the app, prefer [`Self::start_with_clock`] with the
    /// session's `SessionClock`.
    pub fn start(&mut self, epoch: Instant) -> Instant {
        self.start_with_clock(InstantClock::new(epoch));
        epoch
    }

    /// Begin capture on a shared session clock (e.g.
    /// `flickcut_capture::SessionClock`).
    ///
    /// A second start without `stop` is a no-op. The caller keeps
    /// ownership of their clock value.
    pub fn start_with_clock<C: Clock + 'static>(&mut self, clock: C) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        let clock: Arc<dyn Clock> = Arc::new(clock);
        self.clock = Some(clock.clone());
        self.running.store(true, Ordering::Relaxed);

        // Cursor trail thread.
        {
            let running = self.running.clone();
            let trail = self.trail.clone();
            let clock = clock.clone();
            let interval = Duration::from_secs_f64(1.0 / self.cursor_hz);
            thread::Builder::new()
                .name("flickcut-cursor".into())
                .spawn(move || cursor_trail(clock, running, trail, interval))
                .expect("spawn cursor thread");
        }
        // Global mouse/keyboard hooks (rdev blocks; own thread).
        {
            let clock = clock.clone();
            let events = self.events.clone();
            let on_event = self.on_event.clone();
            thread::Builder::new()
                .name("flickcut-hooks".into())
                .spawn(move || global_hooks(clock, events, on_event))
                .expect("spawn hooks thread");
        }
        // Gamepad thread.
        {
            let running = self.running.clone();
            let clock = clock.clone();
            let events = self.events.clone();
            let on_event = self.on_event.clone();
            thread::Builder::new()
                .name("flickcut-gamepad".into())
                .spawn(move || gamepad_loop(clock, running, events, on_event))
                .expect("spawn gamepad thread");
        }
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

    /// Clear events and trail (keeps running state and clock).
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
        self.trail.lock().unwrap().clear();
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
        let clock = self.clock.as_ref().expect("start() before pushing");
        let ev = InputEvent::new(clock, source, action, detail);
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
    clock: Arc<dyn Clock>,
    running: Arc<AtomicBool>,
    trail: Arc<Mutex<Vec<CursorPoint>>>,
    interval: Duration,
) {
    let device = DeviceState::new();
    while running.load(Ordering::Relaxed) {
        let (x, y) = device.get_mouse().coords;
        let (_, t_mono) = clock.stamp();
        trail.lock().unwrap().push(CursorPoint {
            t_ms: t_mono * 1000.0,
            x,
            y,
        });
        thread::sleep(interval);
    }
}

/// Global mouse/keyboard hooks via rdev (press-edge only).
fn global_hooks(
    clock: Arc<dyn Clock>,
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
        push_event(&events, &on_event, InputEvent::new(&clock, source, action, detail));
    }) {
        eprintln!("rdev listen failed: {:?}", e);
    }
}

/// Gamepad buttons/triggers via gilrs (hotplug handled internally).
fn gamepad_loop(
    clock: Arc<dyn Clock>,
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
                InputEvent::new(&clock, "gamepad", action, detail),
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

    fn test_clock(offset: Duration) -> Arc<dyn Clock> {
        Arc::new(InstantClock::new(Instant::now() - offset))
    }

    #[test]
    fn timestamps_use_shared_clock() {
        let clock = test_clock(Duration::from_secs(5));
        let ev = InputEvent::new(&clock, "mouse", "click_left", HashMap::new());
        assert!(
            (4950.0..=5150.0).contains(&ev.t_ms),
            "t_ms={} not ~5000",
            ev.t_ms
        );
        // t_wall is epoch seconds: sane range check.
        assert!(ev.t_wall > 1_700_000_000.0, "t_wall={}", ev.t_wall);
    }

    #[test]
    fn events_stay_chronological() {
        let clock = test_clock(Duration::ZERO);
        let events = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..50 {
            events
                .lock()
                .unwrap()
                .push(InputEvent::new(&clock, "cursor", "move", HashMap::new()));
        }
        let guard = events.lock().unwrap();
        let ts: Vec<f64> = guard.iter().map(|e| e.t_ms).collect();
        let mut sorted = ts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(ts, sorted, "events must be chronological");
    }

    #[test]
    fn trail_points_chronological() {
        let clock = test_clock(Duration::ZERO);
        let pts: Vec<CursorPoint> = (0..20)
            .map(|i| {
                let (_, t_mono) = clock.stamp();
                CursorPoint {
                    t_ms: t_mono * 1000.0,
                    x: 100 + i,
                    y: 200,
                }
            })
            .collect();
        assert_eq!(pts.len(), 20);
        let ts: Vec<f64> = pts.iter().map(|p| p.t_ms).collect();
        let mut sorted = ts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(ts, sorted);
        assert_eq!((pts[0].x, pts[19].x), (100, 119));
    }

    /// A foreign clock (like TAgent's SessionClock) drives the same stamps.
    #[test]
    fn foreign_clock_contract() {
        #[derive(Clone)]
        struct FakeSessionClock {
            t0: Instant,
        }
        impl Clock for FakeSessionClock {
            fn stamp(&self) -> (f64, f64) {
                // Mimics SessionClock: (t_wall, t_mono).
                (1_800_000_000.0, self.t0.elapsed().as_secs_f64())
            }
        }
        let mut t = InputTelemetry::new(None, 30.0);
        t.start_with_clock(FakeSessionClock {
            t0: Instant::now() - Duration::from_millis(2500),
        });
        assert!(t.running());
        t.push_test_event("mouse", "click_left", HashMap::new());
        thread::sleep(Duration::from_millis(50));
        t.push_test_event("gamepad", "btn_down", HashMap::new());
        t.stop();
        let evs = t.events();
        assert_eq!(evs.len(), 2);
        assert!(evs[1].t_ms > evs[0].t_ms);
        assert!((2500.0..=2700.0).contains(&evs[0].t_ms), "t_ms={}", evs[0].t_ms);
        assert!(
            (evs[1].t_ms - evs[0].t_ms - 50.0).abs() < 40.0,
            "delta not ~50ms"
        );
        // t_wall comes from the foreign clock.
        assert_eq!(evs[0].t_wall, 1_800_000_000.0);
    }

    #[test]
    fn telemetry_start_stop_clear() {
        let mut t = InputTelemetry::new(None, 30.0);
        assert!(!t.running());
        let epoch = Instant::now();
        assert_eq!(t.start(epoch), epoch);
        assert!(t.running());
        t.start(Instant::now()); // second start: no-op
        assert!(t.running());
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
        let clock = test_clock(Duration::ZERO);
        let ev = InputEvent::new(&clock, "mouse", "click_left", HashMap::new());
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(json.contains("\"t_ms\""));
        assert!(json.contains("\"t_wall\""));
        assert!(json.contains("click_left"));
    }
}
