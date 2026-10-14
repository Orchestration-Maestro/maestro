//! Empty assistants retain their selected protocol behavior.

use super::cases::assert_rows;
use crate::empty_content::chat::TestResult;

#[test]
fn maestro_empty_assistant_retains_protocol_behavior() -> TestResult {
    assert_rows("maestro_empty_assistant_retains_protocol_behavior")
}

#[test]
fn maestro_empty_arrays_retain_protocol_behavior() -> TestResult {
    assert_rows("maestro_empty_arrays_retain_protocol_behavior")
}

#[test]
fn maestro_empty_strings_retain_protocol_behavior() -> TestResult {
    assert_rows("maestro_empty_strings_retain_protocol_behavior")
}

#[test]
fn maestro_whitespace_inputs_retain_protocol_behavior() -> TestResult {
    assert_rows("maestro_whitespace_inputs_retain_protocol_behavior")
}

#[test]
fn maestro_empty_blocks_keep_protocol_omissions() -> TestResult {
    assert_rows("maestro_empty_blocks_keep_protocol_omissions")
}

#[test]
fn maestro_empty_streams_keep_protocol_terminal_rules() -> TestResult {
    assert_rows("maestro_empty_streams_keep_protocol_terminal_rules")
}
