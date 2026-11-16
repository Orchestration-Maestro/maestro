//! Controlled reasoning and text qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use crate::reasoning_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Native reasoning controls follow the selected model.
#[test]
fn maestro_conversation_selects_native_reasoning_mode() -> TestResult {
    qualification::assert_rows("maestro_conversation_selects_native_reasoning_mode")
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn maestro_conversation_off_omits_reasoning_controls() -> TestResult {
    qualification::assert_rows("maestro_conversation_off_omits_reasoning_controls")
}
