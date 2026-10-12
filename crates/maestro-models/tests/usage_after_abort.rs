//! Usage retained when a controlled invocation is cancelled.

#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bundled.rs"]
mod bundled;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/json.rs"]
mod json;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/messages.rs"]
mod messages;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/response_qualification.rs"]
mod response_qualification;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

#[path = "usage_totals/cases.rs"]
mod cases;

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
