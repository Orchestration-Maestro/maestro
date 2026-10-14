//! Recorded stream observations through public dispatch.

use super::cases::assert_rows;
use crate::model_streaming::chat::TestResult;

#[test]
fn maestro_image_requests_keep_original_bytes() -> TestResult {
    assert_rows("maestro_image_requests_keep_original_bytes")
}
