//! Controlled public dispatch observations.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat_tool_choices::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Completion usage already includes reasoning tokens.
#[test]
fn chat_counts_reasoning_once() -> TestResult {
    qualification::assert_rows("chat_counts_reasoning_once")
}

#[cfg(not(target_arch = "wasm32"))]
/// Chunk and per-choice usage preserve separate cache-write accounting.
#[test]
fn chat_keeps_both_cache_usage_locations() -> TestResult {
    qualification::assert_rows("chat_keeps_both_cache_usage_locations")
}

#[cfg(not(target_arch = "wasm32"))]
/// Cache overlaps, floors and fallback precedence retain recorded accounting.
#[test]
fn chat_normalizes_cache_accounting() -> TestResult {
    qualification::assert_rows("chat_normalizes_cache_accounting")
}
