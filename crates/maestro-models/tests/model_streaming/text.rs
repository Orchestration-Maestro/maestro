//! Completion histories use actual returned assistants.

use super::cases::assert_rows;
use crate::chat::TestResult;

#[test]
fn maestro_completions_continue_with_actual_assistants() -> TestResult {
    assert_rows("maestro_completions_continue_with_actual_assistants")
}

#[test]
fn maestro_stream_text_matches_terminal_result() -> TestResult {
    assert_rows("maestro_stream_text_matches_terminal_result")
}
