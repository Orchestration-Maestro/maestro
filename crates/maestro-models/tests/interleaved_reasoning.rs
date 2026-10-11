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

/// Recorded public observations for reasoning survives tool continuations.
#[test]
fn maestro_reasoning_survives_tool_continuations() -> TestResult {
    qualification::assert_rows("maestro_reasoning_survives_tool_continuations")
}
