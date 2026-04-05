// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Timing and performance measurement utilities.

use std::time::{Duration, Instant};

/// A simple timing guard that records elapsed time on drop.
///
/// # Example
///
/// ```rust
/// use vectorless::util::Timer;
///
/// let timer = Timer::start("indexing");
/// // ... do work ...
/// drop(timer); // Logs elapsed time
/// ```
#[derive(Debug)]
pub struct Timer {
    label: String,
    start: Instant,
    log_on_drop: bool,
}

impl Timer {
    /// Create and start a new timer.
    pub fn start(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            start: Instant::now(),
            log_on_drop: true,
        }
    }

    /// Create a silent timer (doesn't log on drop).
    pub fn silent() -> Self {
        Self {
            label: String::new(),
            start: Instant::now(),
            log_on_drop: false,
        }
    }

    /// Get the elapsed time without stopping.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Get elapsed time in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }

    /// Get elapsed time in seconds.
    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    /// Stop the timer and return the elapsed duration.
    pub fn stop(self) -> Duration {
        let elapsed = self.elapsed();
        if self.log_on_drop {
            tracing::debug!(
                "{} completed in {:.2}ms",
                self.label,
                elapsed.as_secs_f64() * 1000.0
            );
        }
        elapsed
    }

    /// Stop the timer and return elapsed milliseconds.
    pub fn stop_ms(self) -> u64 {
        self.stop().as_millis() as u64
    }

    /// Disable logging on drop.
    pub fn silent_on_drop(mut self) -> Self {
        self.log_on_drop = false;
        self
    }

    /// Reset the timer.
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if self.log_on_drop {
            let elapsed = self.elapsed();
            tracing::debug!(
                "{} completed in {:.2}ms",
                self.label,
                elapsed.as_secs_f64() * 1000.0
            );
        }
    }
}

/// Format a duration for human-readable display.
pub fn format_duration(duration: Duration) -> String {
    let total_ms = duration.as_millis();

    if total_ms < 1000 {
        format!("{}ms", total_ms)
    } else if total_ms < 60_000 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        let secs = duration.as_secs();
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{}m {}s", mins, remaining_secs)
    }
}

/// Format a duration as a compact string.
pub fn format_duration_compact(duration: Duration) -> String {
    let total_ms = duration.as_millis();

    if total_ms < 1000 {
        format!("{}ms", total_ms)
    } else if total_ms < 60_000 {
        format!("{:.1}s", duration.as_secs_f64())
    } else {
        let mins = duration.as_secs() / 60;
        let secs = duration.as_secs() % 60;
        format!("{}:{:02}", mins, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_elapsed() {
        let timer = Timer::silent();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed.as_millis() >= 10);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.50s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
    }

    #[test]
    fn test_format_duration_compact() {
        assert_eq!(format_duration_compact(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration_compact(Duration::from_millis(1500)), "1.5s");
        assert_eq!(format_duration_compact(Duration::from_secs(90)), "1:30");
    }
}
