//! Controlled public message qualification.

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
#[path = "support/message_qualification.rs"]
mod qualification;

#[path = "support/message_probes.rs"]
mod probes;

use chat::TestResult;

/// Subscription calls restore original declaration spellings in tool-end handles.
#[test]
fn maestro_account_restores_original_tool_names() -> TestResult {
    qualification::assert_rows("maestro_account_restores_original_tool_names")
}

/// Unrelated, accented and near-match subscription names remain distinct.
#[test]
fn maestro_account_keeps_unrelated_tool_names() -> TestResult {
    qualification::assert_rows("maestro_account_keeps_unrelated_tool_names")
}

/// Wire history naming retains first-declared spelling; caller isolation is ownership-enforced.
#[test]
fn maestro_account_projects_names_without_mutating_callers() -> TestResult {
    qualification::assert_rows("maestro_account_projects_names_without_mutating_callers")
}

/// Ordinary keys keep wire names while substring-marked tokens select subscription names.
#[test]
fn maestro_account_leaves_key_authenticated_names_plain() -> TestResult {
    qualification::assert_rows("maestro_account_leaves_key_authenticated_names_plain")
}
