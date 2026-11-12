//! Debounced persistence owns captured jobs and completes the final writer.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::state::test_support::api::daemon;
use std::time::{Duration, Instant};

#[test]
fn daemon_hidden_render_attempt_keeps_presentation_cadence_available() {
    let mut app = daemon().unwrap();
    let first = Instant::now();
    app.record_render_attempt(first, true);
    let hidden = first + Duration::from_millis(16);
    app.record_render_attempt(hidden, false);
    let echo = hidden + Duration::from_millis(1);
    assert!(!app.can_render_now(echo));
    assert!(app.can_present_now(echo));
}
