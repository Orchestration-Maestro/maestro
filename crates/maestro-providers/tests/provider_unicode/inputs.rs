//! Authored text and raw-unit observations.

#[cfg(not(target_arch = "wasm32"))]
use crate::provider_unicode::{chat::TestResult, qualification};

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn provider_preserves_emoji_tool_text() -> TestResult {
    qualification::assert_rows("provider_preserves_emoji_tool_text")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn provider_preserves_mixed_comment_text() -> TestResult {
    qualification::assert_rows("provider_preserves_mixed_comment_text")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn provider_preserves_valid_text_roles() -> TestResult {
    qualification::assert_rows("provider_preserves_valid_text_roles")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn sanitizer_retains_pairs_and_removes_lone_units() -> TestResult {
    qualification::assert_rows("sanitizer_retains_pairs_and_removes_lone_units")
}
