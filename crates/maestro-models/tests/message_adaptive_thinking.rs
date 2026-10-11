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

/// Simple catalog mappings and raw effort/display controls retain distinct values.
#[test]
fn maestro_message_preserves_adaptive_effort() -> TestResult {
    qualification::assert_rows("maestro_message_preserves_adaptive_effort")
}

/// Ordered thinking, accumulated signature and exact arithmetic bytes survive streaming.
#[test]
fn maestro_message_streams_signed_arithmetic() -> TestResult {
    qualification::assert_rows("maestro_message_streams_signed_arithmetic")
}
