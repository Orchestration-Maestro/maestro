//! Controlled reasoning and text qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use crate::reasoning_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn missing_results_allow_follow_on_text() -> TestResult {
    qualification::assert_rows("missing_results_allow_follow_on_text")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn missing_results_allow_another_tool_call() -> TestResult {
    qualification::assert_rows("missing_results_allow_another_tool_call")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn pending_tools_flush_at_history_boundaries() -> TestResult {
    qualification::assert_rows("pending_tools_flush_at_history_boundaries")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn failed_assistants_do_not_leave_pending_tools() -> TestResult {
    qualification::assert_rows("failed_assistants_do_not_leave_pending_tools")
}
