//! Controlled public chat qualification.

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
#[path = "support/chat_qualification.rs"]
mod qualification;

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
