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

/// Simple absent reasoning explicitly disables the budget-based model.
#[test]
fn maestro_message_disables_budget_thinking() -> TestResult {
    qualification::assert_rows("maestro_message_disables_budget_thinking")
}

/// Raw missing, false and true thinking differ from simple absent reasoning.
#[test]
fn maestro_message_distinguishes_raw_and_simple_thinking() -> TestResult {
    qualification::assert_rows("maestro_message_distinguishes_raw_and_simple_thinking")
}

/// Simple absent reasoning explicitly disables both qualified adaptive models.
#[test]
fn maestro_message_disables_adaptive_thinking() -> TestResult {
    qualification::assert_rows("maestro_message_disables_adaptive_thinking")
}

/// A closed text-only stream emits no thinking and preserves whole-word pong counts.
#[test]
fn maestro_message_off_stream_contains_only_text() -> TestResult {
    qualification::assert_rows("maestro_message_off_stream_contains_only_text")
}
