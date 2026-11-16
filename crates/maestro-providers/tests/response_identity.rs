//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Returned identity remains attached to the requested route.
#[test]
fn maestro_public_routes_retain_response_ids() -> TestResult {
    qualification::assert_rows("maestro_public_routes_retain_response_ids")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for failed responses keep partial identity.
#[test]
fn maestro_failed_responses_keep_partial_identity() -> TestResult {
    qualification::assert_rows("maestro_failed_responses_keep_partial_identity")
}
