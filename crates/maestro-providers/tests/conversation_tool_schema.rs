//! Controlled reasoning and text qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use crate::reasoning_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Compare the complete recorded public observations.
#[test]
fn conversation_preserves_nested_schema_keywords() -> TestResult {
    qualification::assert_rows("conversation_preserves_nested_schema_keywords")
}
