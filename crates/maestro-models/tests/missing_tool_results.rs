//! Controlled reasoning and text qualification.

use crate::chat;

use crate::reasoning_qualification as qualification;

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
