//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Recorded public observations for public routes honor preabort.
#[test]
fn maestro_public_routes_honor_preabort() -> TestResult {
    qualification::assert_rows("maestro_public_routes_honor_preabort")
}

/// Recorded public observations for cancelled streams keep partials and continue.
#[test]
fn maestro_cancelled_streams_keep_partials_and_continue() -> TestResult {
    qualification::assert_rows("maestro_cancelled_streams_keep_partials_and_continue")
}
