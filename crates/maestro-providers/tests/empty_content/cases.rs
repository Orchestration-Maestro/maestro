//! Closed ownership for recorded empty-content invocations.

#[cfg(not(target_arch = "wasm32"))]
use crate::empty_content::{chat::TestResult, response_qualification::assert_corpus_rows};

#[cfg(not(target_arch = "wasm32"))]
/// Execute only the named test's recorded invocations.
pub(super) fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/empty_content.json"),
        &[
            (
                "crates/maestro-providers/tests/empty_content/assertions.rs::empty_arrays_retain_protocol_behavior",
                23,
            ),
            (
                "crates/maestro-providers/tests/empty_content/assertions.rs::empty_strings_retain_protocol_behavior",
                23,
            ),
            (
                "crates/maestro-providers/tests/empty_content/assertions.rs::whitespace_inputs_retain_protocol_behavior",
                35,
            ),
            (
                "crates/maestro-providers/tests/empty_content/assertions.rs::empty_assistant_retains_protocol_behavior",
                23,
            ),
            (
                "crates/maestro-providers/tests/empty_content/assertions.rs::empty_blocks_keep_protocol_omissions",
                59,
            ),
            (
                "crates/maestro-providers/tests/empty_content/assertions.rs::empty_streams_keep_protocol_terminal_rules",
                23,
            ),
        ],
        test,
    )
}
