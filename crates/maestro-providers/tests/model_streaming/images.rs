//! Recorded stream observations through public dispatch.

#[cfg(not(target_arch = "wasm32"))]
use super::cases::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::model_streaming::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn image_requests_keep_original_bytes() -> TestResult {
    assert_rows("image_requests_keep_original_bytes")
}
