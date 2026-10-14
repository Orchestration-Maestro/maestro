//! Controlled reasoning and text qualification.

use crate::chat;

use crate::reasoning_qualification as qualification;

use chat::TestResult;

/// Compare the complete recorded public observations.
#[test]
fn maestro_shipped_models_disable_unrequested_thinking() -> TestResult {
    qualification::assert_rows("maestro_shipped_models_disable_unrequested_thinking")
}
