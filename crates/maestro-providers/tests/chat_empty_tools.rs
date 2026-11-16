//! Controlled public chat qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Empty declarations do not create a wire tools field without tool history.
#[test]
fn maestro_chat_omits_unused_tool_declarations() -> TestResult {
    qualification::assert_rows("maestro_chat_omits_unused_tool_declarations")
}

#[cfg(not(target_arch = "wasm32"))]
/// Absent and empty declarations retain an empty tools array for tool history.
#[test]
fn maestro_chat_retains_history_required_tools() -> TestResult {
    qualification::assert_rows("maestro_chat_retains_history_required_tools")
}

#[cfg(not(target_arch = "wasm32"))]
/// Gateway requests use conservative fields and the resolved endpoint.
#[test]
fn maestro_gateway_uses_conservative_chat_fields() -> TestResult {
    qualification::assert_rows("maestro_gateway_uses_conservative_chat_fields")
}

#[cfg(not(target_arch = "wasm32"))]
/// Gateway authentication keeps the upstream credential independently.
#[test]
fn maestro_gateway_preserves_upstream_credentials() -> TestResult {
    qualification::assert_rows("maestro_gateway_preserves_upstream_credentials")
}

#[cfg(not(target_arch = "wasm32"))]
/// Caller affinity overrides only the selected header.
#[test]
fn maestro_gateway_sends_session_affinity() -> TestResult {
    qualification::assert_rows("maestro_gateway_sends_session_affinity")
}
