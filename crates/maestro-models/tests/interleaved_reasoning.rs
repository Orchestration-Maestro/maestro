//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for reasoning survives tool continuations.
#[test]
fn maestro_reasoning_survives_tool_continuations() -> TestResult {
    qualification::assert_rows("maestro_reasoning_survives_tool_continuations")
}
