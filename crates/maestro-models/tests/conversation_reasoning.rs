//! Controlled reasoning and text qualification.

use crate::chat;

use crate::reasoning_qualification as qualification;

use chat::TestResult;

/// Native reasoning controls follow the selected model.
#[test]
fn maestro_conversation_selects_native_reasoning_mode() -> TestResult {
    qualification::assert_rows("maestro_conversation_selects_native_reasoning_mode")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_conversation_off_omits_reasoning_controls() -> TestResult {
    qualification::assert_rows("maestro_conversation_off_omits_reasoning_controls")
}
