//! Recorded stream observations through public dispatch.

use super::cases::assert_rows;
use crate::chat::TestResult;

#[test]
fn maestro_stream_keeps_tool_argument_progress() -> TestResult {
    assert_rows("maestro_stream_keeps_tool_argument_progress")
}

#[test]
fn maestro_stream_blocks_keep_independent_indices() -> TestResult {
    assert_rows("maestro_stream_blocks_keep_independent_indices")
}

#[test]
fn maestro_partial_arguments_advance_before_completion() -> TestResult {
    assert_rows("maestro_partial_arguments_advance_before_completion")
}
