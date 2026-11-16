//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Subscription calls restore original declaration spellings in tool-end handles.
#[test]
fn maestro_account_restores_original_tool_names() -> TestResult {
    qualification::assert_rows("maestro_account_restores_original_tool_names")
}

#[cfg(not(target_arch = "wasm32"))]
/// Unrelated, accented and near-match subscription names remain distinct.
#[test]
fn maestro_account_keeps_unrelated_tool_names() -> TestResult {
    qualification::assert_rows("maestro_account_keeps_unrelated_tool_names")
}

#[cfg(not(target_arch = "wasm32"))]
/// History names are canonicalized on the wire; inbound names retain first-declared spelling.
/// Caller isolation is ownership-enforced.
#[test]
fn maestro_account_projects_names_without_mutating_callers() -> TestResult {
    qualification::assert_rows("maestro_account_projects_names_without_mutating_callers")
}

#[cfg(not(target_arch = "wasm32"))]
/// Ordinary keys keep wire names while substring-marked tokens select subscription names.
#[test]
fn maestro_account_leaves_key_authenticated_names_plain() -> TestResult {
    qualification::assert_rows("maestro_account_leaves_key_authenticated_names_plain")
}
