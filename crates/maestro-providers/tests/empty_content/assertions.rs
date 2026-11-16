//! Empty assistants retain their selected protocol behavior.

#[cfg(not(target_arch = "wasm32"))]
use super::cases::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::empty_content::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_empty_assistant_retains_protocol_behavior() -> TestResult {
    assert_rows("maestro_empty_assistant_retains_protocol_behavior")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_empty_arrays_retain_protocol_behavior() -> TestResult {
    assert_rows("maestro_empty_arrays_retain_protocol_behavior")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_empty_strings_retain_protocol_behavior() -> TestResult {
    assert_rows("maestro_empty_strings_retain_protocol_behavior")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_whitespace_inputs_retain_protocol_behavior() -> TestResult {
    assert_rows("maestro_whitespace_inputs_retain_protocol_behavior")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_empty_blocks_keep_protocol_omissions() -> TestResult {
    assert_rows("maestro_empty_blocks_keep_protocol_omissions")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_empty_streams_keep_protocol_terminal_rules() -> TestResult {
    assert_rows("maestro_empty_streams_keep_protocol_terminal_rules")
}
