//! Recorded stream observations through public dispatch.

#[cfg(not(target_arch = "wasm32"))]
use super::cases::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::model_streaming::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_stream_keeps_tool_argument_progress() -> TestResult {
    assert_rows("maestro_stream_keeps_tool_argument_progress")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_stream_blocks_keep_independent_indices() -> TestResult {
    assert_rows("maestro_stream_blocks_keep_independent_indices")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_partial_arguments_advance_before_completion() -> TestResult {
    assert_rows("maestro_partial_arguments_advance_before_completion")
}
