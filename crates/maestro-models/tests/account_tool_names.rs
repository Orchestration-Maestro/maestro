//! Controlled public message qualification.

use crate::chat;
use crate::message_qualification as qualification;

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

/// History names are canonicalized on the wire; inbound names retain first-declared spelling.
/// Caller isolation is ownership-enforced.
#[test]
fn maestro_account_projects_names_without_mutating_callers() -> TestResult {
    qualification::assert_rows("maestro_account_projects_names_without_mutating_callers")
}

/// Ordinary keys keep wire names while substring-marked tokens select subscription names.
#[test]
fn maestro_account_leaves_key_authenticated_names_plain() -> TestResult {
    qualification::assert_rows("maestro_account_leaves_key_authenticated_names_plain")
}
