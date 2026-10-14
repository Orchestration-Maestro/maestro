//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Recorded public observations for tool images stay with function results.
#[test]
fn maestro_tool_images_stay_with_function_results() -> TestResult {
    qualification::assert_rows("maestro_tool_images_stay_with_function_results")
}

/// Recorded public observations for tool outputs preserve content classes.
#[test]
fn maestro_tool_outputs_preserve_content_classes() -> TestResult {
    qualification::assert_rows("maestro_tool_outputs_preserve_content_classes")
}
