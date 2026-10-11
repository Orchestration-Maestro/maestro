//! Authored text and raw-unit observations.

use crate::{chat::TestResult, qualification};

/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_preserves_emoji_tool_text() -> TestResult {
    qualification::assert_rows("maestro_provider_preserves_emoji_tool_text")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_preserves_mixed_comment_text() -> TestResult {
    qualification::assert_rows("maestro_provider_preserves_mixed_comment_text")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_provider_preserves_valid_text_roles() -> TestResult {
    qualification::assert_rows("maestro_provider_preserves_valid_text_roles")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_sanitizer_retains_pairs_and_removes_lone_units() -> TestResult {
    qualification::assert_rows("maestro_sanitizer_retains_pairs_and_removes_lone_units")
}
