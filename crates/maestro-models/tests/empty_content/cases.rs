//! Closed ownership for recorded empty-content invocations.

use crate::empty_content::{chat::TestResult, response_qualification::assert_corpus_rows};

/// Execute only the named test's recorded invocations.
pub(super) fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/empty_content.json"),
        &[
            (
                "crates/maestro-models/tests/empty_content/assertions.rs::maestro_empty_arrays_retain_protocol_behavior",
                23,
            ),
            (
                "crates/maestro-models/tests/empty_content/assertions.rs::maestro_empty_strings_retain_protocol_behavior",
                23,
            ),
            (
                "crates/maestro-models/tests/empty_content/assertions.rs::maestro_whitespace_inputs_retain_protocol_behavior",
                35,
            ),
            (
                "crates/maestro-models/tests/empty_content/assertions.rs::maestro_empty_assistant_retains_protocol_behavior",
                23,
            ),
            (
                "crates/maestro-models/tests/empty_content/assertions.rs::maestro_empty_blocks_keep_protocol_omissions",
                59,
            ),
            (
                "crates/maestro-models/tests/empty_content/assertions.rs::maestro_empty_streams_keep_protocol_terminal_rules",
                23,
            ),
        ],
        test,
    )
}
