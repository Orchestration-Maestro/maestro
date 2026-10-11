//! Controlled public chat qualification.

#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bundled.rs"]
mod bundled;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/json.rs"]
mod json;
#[path = "support/chat_qualification.rs"]
mod qualification;

use chat::TestResult;

/// First distinct reported model and first response identifier stay retained.
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
