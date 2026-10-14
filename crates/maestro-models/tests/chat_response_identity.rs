//! Controlled public chat qualification.

use crate::chat;
use crate::chat_qualification as qualification;

use chat::TestResult;

/// First distinct nonempty reported model and first nonempty response identifier stay retained.
#[test]
fn maestro_chat_retains_routed_model_identity() -> TestResult {
    qualification::assert_rows("maestro_chat_retains_routed_model_identity")
}

/// Provider finish reasons retain partial text and the terminal outcome.
#[test]
fn maestro_chat_preserves_provider_finish_outcomes() -> TestResult {
    qualification::assert_rows("maestro_chat_preserves_provider_finish_outcomes")
}

/// A null wire chunk does not hide the following valid response.
#[test]
fn maestro_chat_ignores_null_wire_chunks() -> TestResult {
    qualification::assert_rows("maestro_chat_ignores_null_wire_chunks")
}
