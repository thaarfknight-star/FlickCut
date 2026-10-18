// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Session clock: the single time base for a recording session.
//!
//! Owned by the capture module. The input-telemetry module (TAgent1) syncs to
//! it: stamp every input event with [`SessionClock::stamp`], and `t_mono`
//! lines up exactly with video frame timestamps.
//!
//! - `t_wall`: seconds since the Unix epoch (wall clock, for file names / logs)
//! - `t_mono`: seconds since session start (monotonic, for pacing and sync)

use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch, as f64.
pub fn wall_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// The session time base. Cheap to clone; all clones share the same origin.
#[derive(Debug, Clone)]
pub struct SessionClock {
    t0_wall: f64,
    t0_mono: Instant,
}

impl SessionClock {
    /// Start a new session clock (t = 0 is now).
    pub fn new() -> Self {
        Self {
            t0_wall: wall_now(),
            t0_mono: Instant::now(),
        }
    }

    /// Wall-clock time of session start (seconds since epoch).
    pub fn t0_wall(&self) -> f64 {
        self.t0_wall
    }

    /// The monotonic instant of session start, for sharing with subsystems
    /// that take a `std::time::Instant` epoch (e.g. input telemetry).
    pub fn t0_instant(&self) -> Instant {
        self.t0_mono
    }

    /// Seconds since session start (monotonic).
    pub fn elapsed_sec(&self) -> f64 {
        self.t0_mono.elapsed().as_secs_f64()
    }

    /// Stamp "now": `(t_wall, t_mono)`.
    ///
    /// This is the coordination point with input telemetry: call it for every
    /// input event and every video frame. `t_mono` values from both sides are
    /// directly comparable.
    pub fn stamp(&self) -> (f64, f64) {
        (wall_now(), self.t0_mono.elapsed().as_secs_f64())
    }
}

impl Default for SessionClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_starts_near_zero() {
        let c = SessionClock::new();
        assert!(c.elapsed_sec() < 1.0);
        let (_, t_mono) = c.stamp();
        assert!(t_mono >= 0.0 && t_mono < 1.0);
    }

    #[test]
    fn clock_is_monotonic() {
        let c = SessionClock::new();
        let mut prev = c.elapsed_sec();
        for _ in 0..100 {
            let cur = c.elapsed_sec();
            assert!(cur >= prev);
            prev = cur;
        }
    }

    #[test]
    fn clock_advances() {
        let c = SessionClock::new();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(c.elapsed_sec() >= 0.05);
    }

    #[test]
    fn wall_time_is_sane() {
        // After 2026-01-01.
        assert!(wall_now() > 1_767_225_600.0);
        let c = SessionClock::new();
        assert!((c.t0_wall() - wall_now()).abs() < 1.0);
    }
}

