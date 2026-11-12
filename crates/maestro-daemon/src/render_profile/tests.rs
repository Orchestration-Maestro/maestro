//! Profiling aggregation without global environment mutation.

use super::*;

#[test]
fn profiling_accumulates_counters_and_duration_extrema_per_window() {
    let mut profiler = RenderProfiler::new();
    profiler.increment("bytes", 3);
    profiler.increment("bytes", 5);
    profiler.duration("write", Duration::from_micros(10));
    profiler.duration("write", Duration::from_micros(30));
    assert_eq!(profiler.counters["bytes"], 8);
    let stats = &profiler.durations["write"];
    assert_eq!(
        (stats.count, stats.total_ns, stats.max_ns),
        (2, 40_000, 30_000)
    );
    profiler.flush_if_due();
    assert_eq!(profiler.counters["bytes"], 8);
    profiler.window_started = Instant::now().checked_sub(Duration::from_secs(2)).unwrap();
    profiler.flush_if_due();
    assert!(profiler.counters.is_empty());
    assert!(profiler.durations.is_empty());
}
