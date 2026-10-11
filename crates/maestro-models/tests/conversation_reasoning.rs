//! Controlled reasoning and text qualification.

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
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/response_qualification.rs"]
mod response_qualification;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

#[path = "support/reasoning_qualification.rs"]
mod qualification;

use chat::TestResult;

/// Native reasoning controls follow the selected model.
#[test]
fn maestro_conversation_selects_native_reasoning_mode() -> TestResult {
    qualification::assert_rows("maestro_conversation_selects_native_reasoning_mode")
}

/// Compare the complete recorded public observations.
#[test]
fn maestro_conversation_off_omits_reasoning_controls() -> TestResult {
    qualification::assert_rows("maestro_conversation_off_omits_reasoning_controls")
}
