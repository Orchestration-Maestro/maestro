//! Completion histories use actual returned assistants.

#[cfg(not(target_arch = "wasm32"))]
use super::cases::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::model_streaming::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn completions_continue_with_actual_assistants() -> TestResult {
    assert_rows("completions_continue_with_actual_assistants")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn stream_text_matches_terminal_result() -> TestResult {
    assert_rows("stream_text_matches_terminal_result")
}
