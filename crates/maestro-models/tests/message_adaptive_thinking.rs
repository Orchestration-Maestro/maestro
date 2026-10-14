//! Controlled public message qualification.

use crate::chat;
use crate::message_qualification as qualification;

use chat::TestResult;

/// Simple catalog mappings and raw effort/display controls retain distinct values.
#[test]
fn maestro_message_preserves_adaptive_effort() -> TestResult {
    qualification::assert_rows("maestro_message_preserves_adaptive_effort")
}

/// Ordered thinking, accumulated signature and exact arithmetic bytes survive streaming.
#[test]
fn maestro_message_streams_signed_arithmetic() -> TestResult {
    qualification::assert_rows("maestro_message_streams_signed_arithmetic")
}
