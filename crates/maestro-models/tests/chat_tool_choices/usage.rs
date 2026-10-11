//! Controlled public dispatch observations.

use crate::{chat::TestResult, qualification};

/// Completion usage already includes reasoning tokens.
#[test]
fn maestro_chat_counts_reasoning_once() -> TestResult {
    qualification::assert_rows("maestro_chat_counts_reasoning_once")
}

/// Chunk and per-choice usage preserve separate cache-write accounting.
#[test]
fn maestro_chat_keeps_both_cache_usage_locations() -> TestResult {
    qualification::assert_rows("maestro_chat_keeps_both_cache_usage_locations")
}

/// Cache overlaps, floors and fallback precedence retain recorded accounting.
#[test]
fn maestro_chat_normalizes_cache_accounting() -> TestResult {
    qualification::assert_rows("maestro_chat_normalizes_cache_accounting")
}
