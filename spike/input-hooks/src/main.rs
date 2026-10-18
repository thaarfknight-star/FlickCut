//! FlickCut input-hooks spike: global mouse/keyboard/gamepad capture in Rust.
//!
//! Mirrors the Python `inputs.py` semantics:
//!   - cursor-position trail at ~30 Hz (the core Recordly signal)
//!   - mouse click + keyboard press/release events (global hooks via rdev)
//!   - gamepad buttons + triggers via gilrs (with hotplug)
//! All events carry `t_ms` — milliseconds since a shared perf epoch.
//!
//! Tauri v2 wiring (see INPUT_HOOKS_REPORT.md): run this in a
//! `std::thread::spawn` inside `.setup()` and forward events with
//! `app_handle.emit("input-event", payload)`.

use device_query::{DeviceQuery, DeviceState};
use gilrs::{Button, Event as GilEvent, Gilrs};
use rdev::{listen, Button as RButton, Event as REvent, EventType, Key};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, Instant};

/// One input event on the shared timeline.
#[derive(Serialize, Clone, Debug)]
struct InputEvent {
    t_ms: f64,
    source: &'static str,
    action: String,
    #[serde(flatten)]
    detail: HashMap<String, serde_json::Value>,
}

fn now_ms(epoch: Instant) -> f64 {
    epoch.elapsed().as_secs_f64() * 1000.0
}

fn emit(
    tx: &mpsc::Sender<InputEvent>,
    epoch: Instant,
    source: &'static str,
    action: impl Into<String>,
    detail: HashMap<String, serde_json::Value>,
) {
    let _ = tx.send(InputEvent {
        t_ms: now_ms(epoch),
        source,
        action: action.into(),
        detail,
    });
}

fn kv(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// Cursor trail thread: poll position at ~30 Hz.
fn cursor_trail(tx: mpsc::Sender<InputEvent>, epoch: Instant, running: Arc<AtomicBool>) {
    let device = DeviceState::new();
    while running.load(Ordering::Relaxed) {
        let (x, y) = device.get_mouse().coords;
        emit(
            tx.clone(),
            epoch,
            "cursor",
            "move",
            kv(&[
                ("x", (x as i64).into()),
                ("y", (y as i64).into()),
            ]),
        );
        thread::sleep(Duration::from_millis(33));
    }
}

/// Global mouse/keyboard hooks via rdev (blocking; runs on its own thread).
fn global_hooks(tx: mpsc::Sender<InputEvent>, epoch: Instant) {
    let txc = tx.clone();
    if let Err(e) = listen(move |ev: REvent| {
        let action: Option<(String, HashMap<String, serde_json::Value>)> = match ev.event_type {
            EventType::ButtonPress(b) => Some((
                format!("click_{}", button_name(b)),
                kv(&[]),
            )),
            EventType::ButtonRelease(_) => None, // press-edge only (like inputs.py)
            EventType::KeyPress(k) => Some((
                "key_down".to_string(),
                kv(&[("key", format!("{:?}", k).into())]),
            )),
            EventType::KeyRelease(_) => None,
            EventType::MouseMove { x, y } => {
                // rdev also streams moves; we use the 30 Hz poller instead
                // to keep a steady trail, so ignore these here.
                let _ = (x, y);
                None
            }
            _ => None,
        };
        if let Some((action, detail)) = action {
            emit(&txc, epoch, "os", action, detail);
        }
    }) {
        eprintln!("rdev listen error: {:?}", e);
    }
}

fn button_name(b: RButton) -> &'static str {
    match b {
        RButton::Left => "left",
        RButton::Right => "right",
        RButton::Middle => "middle",
        _ => "other",
    }
}

/// Gamepad polling via gilrs (~120 Hz edge detection + hotplug).
fn gamepad_loop(tx: mpsc::Sender<InputEvent>, epoch: Instant, running: Arc<AtomicBool>) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gilrs init failed (no gamepad support here?): {}", e);
            return;
        }
    };
    // Fire/attack buttons, mirroring inputs.py FIRE_BUTTONS.
    let fire: [Button; 2] = [Button::RightTrigger, Button::RightTrigger2];
    while running.load(Ordering::Relaxed) {
        while let Some(GilEvent { id, event, .. }) = gilrs.next_event() {
            let (action, mut detail) =
                match event {
                    gilrs::EventType::ButtonPressed(b, _) => (
                        "btn_down".to_string(),
                        kv(&[
                            ("button", format!("{:?}", b).into()),
                            ("fire", fire.contains(&b).into()),
                        ]),
                    ),
                    gilrs::EventType::Connected => {
                        ("connected".to_string(), kv(&[("id", format!("{:?}", id).into())]))
                    }
                    gilrs::EventType::Disconnected => {
                        ("disconnected".to_string(), kv(&[("id", format!("{:?}", id).into())]))
                    }
                    _ => continue,
                };
            detail.insert("pad".to_string(), format!("{:?}", id).into());
            emit(&tx, epoch, "gamepad", action, detail);
        }
        // Drain any ButtonChanged/axis noise we don't need; keep it simple.
        thread::sleep(Duration::from_millis(8));
    }
    let _ = Key::F1; // keep Key import referenced on all platforms
}

fn main() {
    let epoch = Instant::now();
    let running = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel::<InputEvent>();

    // Printer thread: NDJSON to stdout (Tauri would emit() these instead).
    let printer = thread::spawn(move || {
        for ev in rx {
            println!("{}", serde_json::to_string(&ev).unwrap());
        }
    });

    // Ctrl-C -> graceful shutdown.
    let r = running.clone();
    ctrlc_handler(r);

    let tx1 = tx.clone();
    let e1 = epoch;
    let r1 = running.clone();
    thread::spawn(move || cursor_trail(tx1, e1, r1));

    let tx2 = tx.clone();
    let e2 = epoch;
    let r2 = running.clone();
    thread::spawn(move || gamepad_loop(tx2, e2, r2));

    // rdev::listen blocks the current thread.
    global_hooks(tx, epoch);

    running.store(false, Ordering::Relaxed);
    let _ = printer.join();
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    // Minimal Ctrl-C handling without extra deps: a thread watching stdin.
    // (Real app: use the `ctrlc` crate.)
    thread::spawn(move || {
        let mut buf = String::new();
        // Blocks until stdin closes / EOF; not a real signal handler,
        // just keeps the spike dependency-free.
        let _ = std::io::stdin().read_line(&mut buf);
        running.store(false, Ordering::Relaxed);
    });
    // Note: since rdev::listen blocks forever, in practice terminate the
    // process to stop the spike. Tauri integration uses a shutdown flag
    // checked between polls instead.
}
