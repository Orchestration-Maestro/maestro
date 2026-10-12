//! Overflow classification of recorded provider outcomes.

use crate::{cache_overflow_images::assert_rows, chat::TestResult};

/// Returned provider errors classify as context overflow on every delivered route.
#[test]
fn maestro_overflow_errors_keep_provider_routes() -> TestResult {
    assert_rows("maestro_overflow_errors_keep_provider_routes")
}

/// A length stop overflows only with zero output and at least 99% of the window.
#[test]
fn maestro_filled_context_requires_zero_output() -> TestResult {
    assert_rows("maestro_filled_context_requires_zero_output")
}

/// A successful stop overflows only when input plus cached input exceeds the window.
#[test]
fn maestro_successful_overflow_counts_cached_input() -> TestResult {
    assert_rows("maestro_successful_overflow_counts_cached_input")
}

/// Rate-limit and ordinary errors never classify as context overflow.
#[test]
fn maestro_throttling_is_not_context_overflow() -> TestResult {
    assert_rows("maestro_throttling_is_not_context_overflow")
}
