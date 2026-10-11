//! Controlled response identity qualification.

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
#[path = "support/response_qualification.rs"]
mod qualification;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

use chat::TestResult;

/// Recorded public observations for generated tool ids survive both handoffs.
#[test]
fn maestro_generated_tool_ids_survive_both_handoffs() -> TestResult {
    qualification::assert_rows("maestro_generated_tool_ids_survive_both_handoffs")
}

/// Recorded public observations for long prefilled ids keep result pairing.
#[test]
fn maestro_long_prefilled_ids_keep_result_pairing() -> TestResult {
    qualification::assert_rows("maestro_long_prefilled_ids_keep_result_pairing")
}

/// Recorded public observations for handoffs pair each tool occurrence.
#[test]
fn maestro_handoffs_pair_each_tool_occurrence() -> TestResult {
    qualification::assert_rows("maestro_handoffs_pair_each_tool_occurrence")
}
