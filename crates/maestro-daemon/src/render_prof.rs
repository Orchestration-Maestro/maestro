//! Lightweight opt-in render profiling for local performance investigations.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Environment switch for local profiling.
const ENV_VAR: &str = "MAESTRO_RENDER_PROF";

/// Cached process-wide enablement.
static ENABLED: OnceLock<bool> = OnceLock::new();
/// Lazy profiler, without a background timer.
static PROFILER: OnceLock<Mutex<RenderProfiler>> = OnceLock::new();

/// Aggregated duration observations.
#[derive(Default)]
struct DurationStats {
    /// Number of recorded durations.
    count: u64,
    /// Total observed nanoseconds.
    total_ns: u128,
    /// Longest observed duration.
    max_ns: u128,
}

/// Counters and durations in one reporting window.
struct RenderProfiler {
    /// Beginning of this reporting window.
    window_started: Instant,
    /// Counter totals by static name.
    counters: BTreeMap<&'static str, u64>,
    /// Duration totals by static name.
    durations: BTreeMap<&'static str, DurationStats>,
}

impl RenderProfiler {
    /// Start an empty reporting window.
    fn new() -> Self {
        Self {
            window_started: Instant::now(),
            counters: BTreeMap::new(),
            durations: BTreeMap::new(),
        }
    }

    /// Add to a named counter.
    fn increment(&mut self, name: &'static str, value: u64) {
        *self.counters.entry(name).or_default() += value;
    }

    /// Record one named duration.
    fn duration(&mut self, name: &'static str, duration: Duration) {
        let stats = self.durations.entry(name).or_default();
        let ns = duration.as_nanos();
        stats.count += 1;
        stats.total_ns += ns;
        stats.max_ns = stats.max_ns.max(ns);
    }

    /// Log and clear a window after at least one second.
    fn flush_if_due(&mut self) {
        let elapsed = self.window_started.elapsed();
        if elapsed < Duration::from_secs(1) {
            return;
        }

        let counters = self
            .counters
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        let durations = self
            .durations
            .iter()
            .map(|(name, stats)| {
                let avg_us = if stats.count == 0 {
                    0
                } else {
                    stats.total_ns / u128::from(stats.count) / 1_000
                };
                let max_us = stats.max_ns / 1_000;
                format!(
                    "{name}=count:{} avg_us:{} max_us:{}",
                    stats.count, avg_us, max_us
                )
            })
            .collect::<Vec<_>>()
            .join(",");

        tracing::info!(
            event = "render.prof",
            window_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            counters = %counters,
            durations = %durations,
            "render profiler window"
        );

        self.window_started = Instant::now();
        self.counters.clear();
        self.durations.clear();
    }
}

/// Whether profiling is enabled for this process.
pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var(ENV_VAR)
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
    })
}

/// Update the lazy profiler only when enabled.
fn with_profiler(update: impl FnOnce(&mut RenderProfiler)) {
    if !enabled() {
        return;
    }
    let profiler = PROFILER.get_or_init(|| Mutex::new(RenderProfiler::new()));
    if let Ok(mut profiler) = profiler.lock() {
        update(&mut profiler);
    }
}

/// Add a nonzero value to a named counter.
pub fn counter(name: &'static str, value: u64) {
    if value == 0 {
        return;
    }
    with_profiler(|profiler| profiler.increment(name, value));
}

/// Increment a named event counter.
pub fn event(name: &'static str) {
    counter(name, 1);
}

/// Record one named duration.
pub fn duration(name: &'static str, duration: Duration) {
    with_profiler(|profiler| profiler.duration(name, duration));
}

/// Start a measurement only when profiling is enabled.
pub fn timer() -> Option<Instant> {
    enabled().then(Instant::now)
}

/// Record elapsed time when a measurement was started.
pub fn duration_since(name: &'static str, started: Option<Instant>) {
    if let Some(started) = started {
        duration(name, started.elapsed());
    }
}

/// Log and clear a window after at least one second.
pub fn flush_if_due() {
    with_profiler(RenderProfiler::flush_if_due);
}

#[cfg(test)]
mod tests;
