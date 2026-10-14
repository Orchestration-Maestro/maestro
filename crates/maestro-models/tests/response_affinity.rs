//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Recorded public observations for response cache keys follow retention.
#[test]
fn maestro_response_cache_keys_follow_retention() -> TestResult {
    qualification::assert_rows("maestro_response_cache_keys_follow_retention")
}
