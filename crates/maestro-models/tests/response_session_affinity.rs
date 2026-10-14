//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Recorded public observations for account sse keeps session affinity.
#[test]
fn maestro_account_sse_keeps_session_affinity() -> TestResult {
    qualification::assert_rows("maestro_account_sse_keeps_session_affinity")
}
