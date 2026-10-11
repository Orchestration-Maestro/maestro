//! Controlled public message qualification.

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
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/messages.rs"]
mod messages;
#[path = "support/message_qualification.rs"]
mod qualification;

#[path = "support/message_probes.rs"]
mod probes;

use chat::TestResult;

/// Catalog account routes retain bearer, initiator and image-specific headers.
#[test]
fn maestro_account_routes_message_authentication() -> TestResult {
    qualification::assert_rows("maestro_account_routes_message_authentication")
}

/// Interleaved beta options depend on model family independently of thinking enablement.
#[test]
fn maestro_account_selects_interleaved_beta_independently() -> TestResult {
    qualification::assert_rows("maestro_account_selects_interleaved_beta_independently")
}
