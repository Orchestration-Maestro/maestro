//! Usage retained when a controlled invocation is cancelled.

use crate::chat;

use crate::usage_cases as cases;

#[test]
fn maestro_abort_retains_reported_usage() -> chat::TestResult {
    cases::assert_rows("maestro_abort_retains_reported_usage")
}

#[test]
fn maestro_abort_before_usage_keeps_zero_counters() -> chat::TestResult {
    cases::assert_rows("maestro_abort_before_usage_keeps_zero_counters")
}

#[test]
fn maestro_abort_keeps_each_reported_category() -> chat::TestResult {
    cases::assert_rows("maestro_abort_keeps_each_reported_category")
}

#[test]
fn maestro_final_only_usage_is_not_fabricated() -> chat::TestResult {
    cases::assert_rows("maestro_final_only_usage_is_not_fabricated")
}
