//! Recorded stream observations through public dispatch.

use super::cases::assert_rows;
use crate::chat::TestResult;

#[test]
fn maestro_stream_thinking_keeps_controls_and_signatures() -> TestResult {
    assert_rows("maestro_stream_thinking_keeps_controls_and_signatures")
}

#[test]
fn maestro_tool_results_drive_follow_on_turns() -> TestResult {
    assert_rows("maestro_tool_results_drive_follow_on_turns")
}
