//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Returned identity remains attached to the requested route.
#[test]
fn maestro_public_routes_retain_response_ids() -> TestResult {
    qualification::assert_rows("maestro_public_routes_retain_response_ids")
}

/// Recorded public observations for failed responses keep partial identity.
#[test]
fn maestro_failed_responses_keep_partial_identity() -> TestResult {
    qualification::assert_rows("maestro_failed_responses_keep_partial_identity")
}
