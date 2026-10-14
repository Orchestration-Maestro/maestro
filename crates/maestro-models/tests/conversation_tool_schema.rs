//! Controlled reasoning and text qualification.

use crate::chat;

use crate::reasoning_qualification as qualification;

use chat::TestResult;

/// Compare the complete recorded public observations.
#[test]
fn maestro_conversation_preserves_nested_schema_keywords() -> TestResult {
    qualification::assert_rows("maestro_conversation_preserves_nested_schema_keywords")
}
