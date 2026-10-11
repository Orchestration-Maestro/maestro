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

/// Recorded public observations for public routes honor preabort.
#[test]
fn maestro_public_routes_honor_preabort() -> TestResult {
    qualification::assert_rows("maestro_public_routes_honor_preabort")
}

/// Recorded public observations for cancelled streams keep partials and continue.
#[test]
fn maestro_cancelled_streams_keep_partials_and_continue() -> TestResult {
    qualification::assert_rows("maestro_cancelled_streams_keep_partials_and_continue")
}
