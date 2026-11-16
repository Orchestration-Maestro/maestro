//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for response cache keys follow retention.
#[test]
fn maestro_response_cache_keys_follow_retention() -> TestResult {
    qualification::assert_rows("maestro_response_cache_keys_follow_retention")
}
