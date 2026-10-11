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

/// Empty declarations do not create a wire tools field without tool history.
#[test]
fn maestro_chat_omits_unused_tool_declarations() -> TestResult {
    qualification::assert_rows("maestro_chat_omits_unused_tool_declarations")
}

/// Absent and empty declarations retain an empty tools array for tool history.
#[test]
fn maestro_chat_retains_history_required_tools() -> TestResult {
    qualification::assert_rows("maestro_chat_retains_history_required_tools")
}

/// Gateway requests use conservative fields and the resolved endpoint.
#[test]
fn maestro_gateway_uses_conservative_chat_fields() -> TestResult {
    qualification::assert_rows("maestro_gateway_uses_conservative_chat_fields")
}

/// Gateway authentication keeps the upstream credential independently.
#[test]
fn maestro_gateway_preserves_upstream_credentials() -> TestResult {
    qualification::assert_rows("maestro_gateway_preserves_upstream_credentials")
}

/// Caller affinity overrides only the selected header.
#[test]
fn maestro_gateway_sends_session_affinity() -> TestResult {
    qualification::assert_rows("maestro_gateway_sends_session_affinity")
}
