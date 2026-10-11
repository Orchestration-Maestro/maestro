//! Controlled reasoning and text qualification.

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

#[path = "support/reasoning_qualification.rs"]
mod qualification;

use chat::TestResult;

/// Compare the complete recorded public observations.
#[test]
fn maestro_missing_results_allow_follow_on_text() -> TestResult {
    qualification::assert_rows("maestro_missing_results_allow_follow_on_text")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_missing_results_allow_another_tool_call() -> TestResult {
    qualification::assert_rows("maestro_missing_results_allow_another_tool_call")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_pending_tools_flush_at_history_boundaries() -> TestResult {
    qualification::assert_rows("maestro_pending_tools_flush_at_history_boundaries")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_failed_assistants_do_not_leave_pending_tools() -> TestResult {
    qualification::assert_rows("maestro_failed_assistants_do_not_leave_pending_tools")
}
