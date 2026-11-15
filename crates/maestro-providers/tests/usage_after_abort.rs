//! Usage retained when a controlled invocation is cancelled.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use crate::usage_cases as cases;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_abort_retains_reported_usage() -> chat::TestResult {
    cases::assert_rows("maestro_abort_retains_reported_usage")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_abort_before_usage_keeps_zero_counters() -> chat::TestResult {
    cases::assert_rows("maestro_abort_before_usage_keeps_zero_counters")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_abort_keeps_each_reported_category() -> chat::TestResult {
    cases::assert_rows("maestro_abort_keeps_each_reported_category")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_final_only_usage_is_not_fabricated() -> chat::TestResult {
    cases::assert_rows("maestro_final_only_usage_is_not_fabricated")
}
