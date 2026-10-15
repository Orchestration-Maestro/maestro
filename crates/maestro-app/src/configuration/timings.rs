//! Startup timing samples and diagnostic output.

use std::io;

/// One retained startup sample.
struct Timing {
    /// Owned diagnostic label.
    label: String,
    /// Milliseconds since the previous observation.
    ms: f64,
}
/// Caller-owned startup profiling state.
pub struct Timings {
    /// Whether profiling was enabled at construction.
    enabled: bool,
    /// The previous clock reading.
    last_time: f64,
    /// Samples in observation order.
    samples: Vec<Timing>,
}
impl Timings {
    /// Capture the initial clock, enabling profiling only for exact `1`.
    pub fn new(environment: Option<&str>, now: impl FnOnce() -> f64) -> Self {
        Self {
            enabled: environment == Some("1"),
            last_time: now(),
            samples: Vec::new(),
        }
    }
    /// Clear enabled samples and capture a new initial time.
    pub fn reset_timings(&mut self, now: impl FnOnce() -> f64) {
        if !self.enabled {
            return;
        }
        self.samples.clear();
        self.last_time = now();
    }
    /// Record an enabled sample with its unmodified label.
    pub fn time(&mut self, label: &str, now: impl FnOnce() -> f64) {
        if !self.enabled {
            return;
        }
        let now = now();
        self.samples.push(Timing {
            label: label.to_owned(),
            ms: now - self.last_time,
        });
        self.last_time = now;
    }
    /// Print enabled samples without consuming them.
    ///
    /// # Errors
    /// Returns the original writer error.
    pub fn print_timings(&self, stderr: &mut dyn io::Write) -> io::Result<()> {
        if !self.enabled || self.samples.is_empty() {
            return Ok(());
        }
        writeln!(stderr, "\n--- Startup Timings ---")?;
        let mut buffer = ryu_js::Buffer::new();
        for t in &self.samples {
            writeln!(stderr, "  {}: {}ms", t.label, buffer.format(t.ms))?;
        }
        let total = self.samples.iter().fold(0.0, |sum, t| sum + t.ms);
        writeln!(stderr, "  TOTAL: {}ms", buffer.format(total))?;
        writeln!(stderr, "------------------------\n")
    }
}
