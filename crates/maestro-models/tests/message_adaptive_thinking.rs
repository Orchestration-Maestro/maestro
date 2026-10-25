//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Simple catalog mappings and raw effort/display controls retain distinct values.
#[test]
fn maestro_message_preserves_adaptive_effort() -> TestResult {
    qualification::assert_rows("maestro_message_preserves_adaptive_effort")
}

#[cfg(not(target_arch = "wasm32"))]
/// Ordered thinking, accumulated signature and exact arithmetic bytes survive streaming.
#[test]
fn maestro_message_streams_signed_arithmetic() -> TestResult {
    qualification::assert_rows("maestro_message_streams_signed_arithmetic")
}
