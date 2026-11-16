//! Closed ownership for recorded streaming invocations.

#[cfg(not(target_arch = "wasm32"))]
use crate::model_streaming::{chat::TestResult, response_qualification::assert_corpus_rows};

#[cfg(not(target_arch = "wasm32"))]
/// Execute only the named test's recorded invocations.
pub(super) fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        &std::fs::read_to_string(format!(
            "{}/tests/fixtures/model_streaming.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("recorded fixture"),
        &[
            (
                "crates/maestro-providers/tests/model_streaming/text.rs::completions_continue_with_actual_assistants",
                66,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/tools.rs::stream_keeps_tool_argument_progress",
                33,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/text.rs::stream_text_matches_terminal_result",
                33,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/images.rs::image_requests_keep_original_bytes",
                15,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/reasoning.rs::stream_thinking_keeps_controls_and_signatures",
                27,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/reasoning.rs::tool_results_drive_follow_on_turns",
                87,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/tools.rs::stream_blocks_keep_independent_indices",
                6,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/tools.rs::partial_arguments_advance_before_completion",
                6,
            ),
        ],
        test,
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// Execute the native socket corpus with its own closed case inventory.
pub(super) fn assert_socket_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/socket_model_streaming.json"),
        &[
            (
                "crates/maestro-providers/tests/model_streaming/socket.rs::socket_completions_continue_with_actual_assistants",
                2,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/socket.rs::socket_keeps_tool_argument_progress",
                1,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/socket.rs::socket_text_matches_terminal_result",
                1,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/socket.rs::socket_thinking_keeps_controls_and_signatures",
                1,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/socket.rs::socket_tool_results_drive_follow_on_turns",
                3,
            ),
            (
                "crates/maestro-providers/tests/model_streaming/socket.rs::socket_image_requests_keep_original_bytes",
                1,
            ),
        ],
        test,
    )
}
