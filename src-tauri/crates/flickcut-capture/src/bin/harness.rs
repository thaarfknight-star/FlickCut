//! flickcut-harness: test driver for the capture module (no UI needed).
//!
//!   flickcut-harness list
//!   flickcut-harness record <source_id> <seconds> <output.mp4>
//!
//! Prints JSON to stdout; exits non-zero with the error on stderr.

use flickcut_capture::{list_sources, Session};
use std::path::Path;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn cmd_list() {
    let l = list_sources();
    let screens: Vec<String> = l
        .screens
        .iter()
        .map(|s| format!("{{\"id\":\"{}\",\"name\":\"{}\"}}", esc(&s.id), esc(&s.name)))
        .collect();
    let windows: Vec<String> = l
        .windows
        .iter()
        .map(|s| format!("{{\"id\":\"{}\",\"title\":\"{}\"}}", esc(&s.id), esc(&s.name)))
        .collect();
    println!(
        "{{\"screens\":[{}],\"windows\":[{}]}}",
        screens.join(","),
        windows.join(",")
    );
}

fn cmd_record(source_id: &str, seconds: u64, output: &str) {
    let session = match Session::start(source_id, 30, Path::new(output)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("start failed: {e}");
            std::process::exit(1);
        }
    };
    let session_id = session.id().to_string();
    // Demonstrate the shared clock: stamp here like telemetry would.
    let clock = session.clock();
    let (tw, tm) = clock.stamp();
    eprintln!("session {session_id} clock t0_wall={tw:.3} (telemetry syncs via clock.stamp())");
    let _ = tm;
    std::thread::sleep(std::time::Duration::from_secs(seconds));
    match session.stop() {
        Ok(s) => println!(
            concat!(
                "{{\"session_id\":\"{}\",\"video_path\":\"{}\",\"duration_sec\":{:.3},",
                "\"frames_captured\":{},\"frames_written\":{},\"frames_dropped\":{},",
                "\"width\":{},\"height\":{},\"fps_target\":{}}}"
            ),
            esc(&s.session_id),
            esc(&s.video_path.to_string_lossy()),
            s.duration_sec,
            s.frames_captured,
            s.frames_written,
            s.frames_dropped,
            s.width,
            s.height,
            s.fps_target,
        ),
        Err(e) => {
            eprintln!("stop failed: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("list") => cmd_list(),
        Some("record") => {
            let source = args.get(2).map(|s| s.as_str()).unwrap_or("screen:0");
            let seconds: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
            let output = args.get(4).map(|s| s.as_str()).unwrap_or("flickcut-test.mp4");
            cmd_record(source, seconds, output);
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  flickcut-harness list");
            eprintln!("  flickcut-harness record [source_id] [seconds] [output.mp4]");
            std::process::exit(2);
        }
    }
}
