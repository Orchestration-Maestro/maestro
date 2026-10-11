//! Closed ownership for recorded streaming invocations.

use crate::{chat::TestResult, response_qualification::assert_corpus_rows};

/// Execute only the named test's recorded invocations.
pub(super) fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/model_streaming.json"),
        &[
            (
                "crates/maestro-models/tests/model_streaming/text.rs::maestro_completions_continue_with_actual_assistants",
                66,
            ),
            (
                "crates/maestro-models/tests/model_streaming/tools.rs::maestro_stream_keeps_tool_argument_progress",
                33,
            ),
            (
                "crates/maestro-models/tests/model_streaming/text.rs::maestro_stream_text_matches_terminal_result",
                33,
            ),
            (
                "crates/maestro-models/tests/model_streaming/images.rs::maestro_image_requests_keep_original_bytes",
                15,
            ),
            (
                "crates/maestro-models/tests/model_streaming/reasoning.rs::maestro_stream_thinking_keeps_controls_and_signatures",
                27,
            ),
            (
                "crates/maestro-models/tests/model_streaming/reasoning.rs::maestro_tool_results_drive_follow_on_turns",
                87,
            ),
            (
                "crates/maestro-models/tests/model_streaming/tools.rs::maestro_stream_blocks_keep_independent_indices",
                6,
            ),
            (
                "crates/maestro-models/tests/model_streaming/tools.rs::maestro_partial_arguments_advance_before_completion",
                6,
            ),
        ],
        test,
    )
}

/// Execute the native socket corpus with its own closed case inventory.
pub(super) fn assert_socket_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/socket_model_streaming.json"),
        &[
            (
                "crates/maestro-models/tests/model_streaming/socket.rs::maestro_socket_completions_continue_with_actual_assistants",
                2,
            ),
            (
                "crates/maestro-models/tests/model_streaming/socket.rs::maestro_socket_keeps_tool_argument_progress",
                1,
            ),
            (
                "crates/maestro-models/tests/model_streaming/socket.rs::maestro_socket_text_matches_terminal_result",
                1,
            ),
            (
                "crates/maestro-models/tests/model_streaming/socket.rs::maestro_socket_thinking_keeps_controls_and_signatures",
                1,
            ),
            (
                "crates/maestro-models/tests/model_streaming/socket.rs::maestro_socket_tool_results_drive_follow_on_turns",
                3,
            ),
            (
                "crates/maestro-models/tests/model_streaming/socket.rs::maestro_socket_image_requests_keep_original_bytes",
                1,
            ),
        ],
        test,
    )
}
