//! flickcut-capture-spike: minimal native-capture feasibility demo.
//!
//! Usage:
//!   flickcut-capture-spike list                  - enumerate capture targets
//!   flickcut-capture-spike shot [target] [out]   - capture N frames to PNG
//!
//! Frames carry wall + monotonic timestamps (see capture.rs), the same
//! contract the Python recorder's sidecar JSON provided.

mod capture;

use capture::{capture_frame, list_targets, now_mono, now_wall};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("list") => cmd_list(),
        Some("shot") => {
            let target: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let n: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
            cmd_shot(target, n);
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  flickcut-capture-spike list");
            eprintln!("  flickcut-capture-spike shot [target_id] [frames]");
            std::process::exit(2);
        }
    }
}

fn cmd_list() {
    match list_targets() {
        Ok(targets) => {
            println!("targets: {}", targets.len());
            for t in targets {
                println!(
                    "  [{}] {:?} {} ({}x{} @ {},{})",
                    t.id, t.kind, t.name, t.width, t.height, t.x, t.y
                );
            }
        }
        Err(e) => {
            eprintln!("list_targets failed (expected on headless hosts): {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_shot(target: u32, n: usize) {
    let mut stamps = Vec::new();
    for i in 0..n {
        let t0 = now_mono();
        match capture_frame(target) {
            Ok(f) => {
                let dt = now_mono() - t0;
                println!(
                    "frame {i}: {}x{} t_wall={:.3} t_mono={:.3} grab={:.1}ms",
                    f.width,
                    f.height,
                    f.t_wall,
                    f.t_mono,
                    dt * 1000.0
                );
                stamps.push((f.t_wall, f.t_mono));
                if let Some(img) =
                    image::RgbaImage::from_raw(f.width, f.height, f.pixels_rgba)
                {
                    let path = format!("flickcut-spike-frame-{i}.png");
                    if let Err(e) = img.save(&path) {
                        eprintln!("save {path} failed: {e}");
                    } else {
                        println!("  saved {path}");
                    }
                }
                let _ = now_wall();
            }
            Err(e) => {
                eprintln!("capture_frame failed (expected on headless hosts): {e}");
                std::process::exit(1);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if stamps.len() >= 2 {
        let dt = stamps.last().unwrap().1 - stamps.first().unwrap().1;
        let fps = (stamps.len() - 1) as f64 / dt.max(1e-6);
        println!("measured pacing: {fps:.1} fps over {dt:.3}s");
        let mono_sorted = stamps.windows(2).all(|w| w[1].1 >= w[0].1);
        println!("timestamps monotonic: {mono_sorted}");
    }
}
