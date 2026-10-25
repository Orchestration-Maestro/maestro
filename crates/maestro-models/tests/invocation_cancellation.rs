//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for public routes honor preabort.
#[test]
fn maestro_public_routes_honor_preabort() -> TestResult {
    qualification::assert_rows("maestro_public_routes_honor_preabort")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for cancelled streams keep partials and continue.
#[test]
fn maestro_cancelled_streams_keep_partials_and_continue() -> TestResult {
    qualification::assert_rows("maestro_cancelled_streams_keep_partials_and_continue")
}
