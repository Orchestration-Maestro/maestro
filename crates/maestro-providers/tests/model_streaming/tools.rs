//! Recorded stream observations through public dispatch.

#[cfg(not(target_arch = "wasm32"))]
use super::cases::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::model_streaming::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stream_keeps_tool_argument_progress() -> TestResult {
    assert_rows("stream_keeps_tool_argument_progress")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stream_blocks_keep_independent_indices() -> TestResult {
    assert_rows("stream_blocks_keep_independent_indices")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn partial_arguments_advance_before_completion() -> TestResult {
    assert_rows("partial_arguments_advance_before_completion")
}
