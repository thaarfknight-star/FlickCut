use flickcut_moments::{analyze, DetailValue, InputEvent};

fn clicks(start_ms: f64, n: usize, gap_ms: f64, x: Option<f64>, y: Option<f64>,
           source: &str, action: &str, fire: Option<bool>) -> Vec<InputEvent> {
    (0..n).map(|i| {
        let mut e = InputEvent::new(start_ms + i as f64 * gap_ms, source, action);
        if let (Some(x), Some(y)) = (x, y) {
            e = e.with_detail("x", DetailValue::Float(x + i as f64 * 10.0));
            e = e.with_detail("y", DetailValue::Float(y + i as f64 * 5.0));
        }
        if let Some(f) = fire {
            e = e.with_detail("fire", DetailValue::Bool(f));
        }
        e
    }).collect()
}

fn main() {
    let mut evs = clicks(10000.0, 4, 150.0, Some(1440.0), Some(810.0), "mouse", "click_left", None);
    evs.extend(clicks(20000.0, 2, 200.0, Some(100.0), Some(100.0), "mouse", "click_left", None));
    evs.extend(clicks(30000.0, 10, 300.0, None, None, "mouse", "click_left", None));
    evs.extend(clicks(70000.0, 4, 150.0, None, None, "gamepad", "btn_down", Some(true)));
    let markers = analyze(&evs, Some((1920.0, 1080.0)));
    print!("[");
    for (i, m) in markers.iter().enumerate() {
        if i > 0 { print!(","); }
        print!(
            "\n {{\"timestamp\": {}, \"type\": \"{}\", \"intensity\": {}, \"duration\": {}, \"x\": {}, \"y\": {}}}",
            m.timestamp, m.kind, m.intensity, m.duration, m.x, m.y
        );
    }
    println!("\n]");
}
