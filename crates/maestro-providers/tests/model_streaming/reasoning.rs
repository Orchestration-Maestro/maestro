//! Recorded stream observations through public dispatch.

#[cfg(not(target_arch = "wasm32"))]
use super::cases::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::model_streaming::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stream_thinking_keeps_controls_and_signatures() -> TestResult {
    assert_rows("stream_thinking_keeps_controls_and_signatures")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_results_drive_follow_on_turns() -> TestResult {
    assert_rows("tool_results_drive_follow_on_turns")
}
