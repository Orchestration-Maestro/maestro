//! Controlled response identity qualification.

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
#[path = "support/response_qualification.rs"]
mod qualification;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

use chat::TestResult;

/// Returned identity remains attached to the requested route.
#[test]
fn maestro_public_routes_retain_response_ids() -> TestResult {
    qualification::assert_rows("maestro_public_routes_retain_response_ids")
}

/// Recorded public observations for failed responses keep partial identity.
#[test]
fn maestro_failed_responses_keep_partial_identity() -> TestResult {
    qualification::assert_rows("maestro_failed_responses_keep_partial_identity")
}
