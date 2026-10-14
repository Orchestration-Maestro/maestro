//! Controlled response identity qualification.

use crate::chat;
use crate::response_qualification as qualification;

use chat::TestResult;

/// Recorded public observations for reasoning survives tool continuations.
#[test]
fn maestro_reasoning_survives_tool_continuations() -> TestResult {
    qualification::assert_rows("maestro_reasoning_survives_tool_continuations")
}
