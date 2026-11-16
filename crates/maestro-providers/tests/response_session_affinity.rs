//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for account sse keeps session affinity.
#[test]
fn maestro_account_sse_keeps_session_affinity() -> TestResult {
    qualification::assert_rows("maestro_account_sse_keeps_session_affinity")
}
