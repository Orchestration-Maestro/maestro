//! Controlled response identity qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for tool images stay with function results.
#[test]
fn maestro_tool_images_stay_with_function_results() -> TestResult {
    qualification::assert_rows("maestro_tool_images_stay_with_function_results")
}

#[cfg(not(target_arch = "wasm32"))]
/// Recorded public observations for tool outputs preserve content classes.
#[test]
fn maestro_tool_outputs_preserve_content_classes() -> TestResult {
    qualification::assert_rows("maestro_tool_outputs_preserve_content_classes")
}
