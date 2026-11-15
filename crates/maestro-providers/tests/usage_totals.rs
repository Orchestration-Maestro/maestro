//! Token totals and costs from controlled provider usage reports.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use crate::usage_cases as cases;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_usage_preserves_provider_total() -> chat::TestResult {
    cases::assert_rows("maestro_usage_preserves_provider_total")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_usage_avoids_cache_and_reasoning_double_count() -> chat::TestResult {
    cases::assert_rows("maestro_chat_usage_avoids_cache_and_reasoning_double_count")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_chat_usage_prefers_top_level_then_choice() -> chat::TestResult {
    cases::assert_rows("maestro_chat_usage_prefers_top_level_then_choice")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_message_usage_updates_only_reported_categories() -> chat::TestResult {
    cases::assert_rows("maestro_message_usage_updates_only_reported_categories")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_response_usage_preserves_reported_totals() -> chat::TestResult {
    cases::assert_rows("maestro_response_usage_preserves_reported_totals")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_conversation_usage_uses_total_or_fallback() -> chat::TestResult {
    cases::assert_rows("maestro_conversation_usage_uses_total_or_fallback")
}
