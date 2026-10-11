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

/// Long-retention probes preserve each selected catalog route and eager compatibility.
#[test]
fn maestro_message_probes_long_retention_per_provider() -> TestResult {
    qualification::assert_rows("maestro_message_probes_long_retention_per_provider")
}

/// Retention markers follow final eligible blocks rather than earlier user turns.
#[test]
fn maestro_message_places_retention_markers() -> TestResult {
    qualification::assert_rows("maestro_message_places_retention_markers")
}

/// Unequal eager and long compatibility switches select independent wire fields.
#[test]
fn maestro_message_keeps_compatibility_policies_independent() -> TestResult {
    qualification::assert_rows("maestro_message_keeps_compatibility_policies_independent")
}
