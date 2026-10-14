//! Native socket completions retain actual returned history.

use super::cases::assert_socket_rows;
use crate::model_streaming::chat::TestResult;

#[test]
fn maestro_socket_completions_continue_with_actual_assistants() -> TestResult {
    assert_socket_rows("maestro_socket_completions_continue_with_actual_assistants")
}

#[test]
fn maestro_socket_text_matches_terminal_result() -> TestResult {
    assert_socket_rows("maestro_socket_text_matches_terminal_result")
}

#[test]
fn maestro_socket_keeps_tool_argument_progress() -> TestResult {
    assert_socket_rows("maestro_socket_keeps_tool_argument_progress")
}

#[test]
fn maestro_socket_thinking_keeps_controls_and_signatures() -> TestResult {
    assert_socket_rows("maestro_socket_thinking_keeps_controls_and_signatures")
}

#[test]
fn maestro_socket_tool_results_drive_follow_on_turns() -> TestResult {
    assert_socket_rows("maestro_socket_tool_results_drive_follow_on_turns")
}

#[test]
fn maestro_socket_image_requests_keep_original_bytes() -> TestResult {
    assert_socket_rows("maestro_socket_image_requests_keep_original_bytes")
}
