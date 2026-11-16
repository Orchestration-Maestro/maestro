//! Controlled public chat qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// The cache marker selects text while preserving array and invalid-content branches.
#[test]
fn maestro_cache_marker_selects_last_user_text() -> TestResult {
    qualification::assert_rows("maestro_cache_marker_selects_last_user_text")
}

#[cfg(not(target_arch = "wasm32"))]
/// Two controlled routed completions retain their own requests and cache usage.
#[test]
fn maestro_routed_chat_retains_cache_writes() -> TestResult {
    qualification::assert_rows("maestro_routed_chat_retains_cache_writes")
}
