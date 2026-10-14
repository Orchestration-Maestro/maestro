//! Controlled public chat qualification.

use crate::chat;
use crate::chat_qualification as qualification;

use chat::TestResult;

/// The cache marker selects text while preserving array and invalid-content branches.
#[test]
fn maestro_cache_marker_selects_last_user_text() -> TestResult {
    qualification::assert_rows("maestro_cache_marker_selects_last_user_text")
}

/// Two controlled routed completions retain their own requests and cache usage.
#[test]
fn maestro_routed_chat_retains_cache_writes() -> TestResult {
    qualification::assert_rows("maestro_routed_chat_retains_cache_writes")
}
