//! Token totals and costs from controlled provider usage reports.

use crate::chat;

use crate::usage_cases as cases;

#[test]
fn maestro_usage_preserves_provider_total() -> chat::TestResult {
    cases::assert_rows("maestro_usage_preserves_provider_total")
}

#[test]
fn maestro_chat_usage_avoids_cache_and_reasoning_double_count() -> chat::TestResult {
    cases::assert_rows("maestro_chat_usage_avoids_cache_and_reasoning_double_count")
}

#[test]
fn maestro_chat_usage_prefers_top_level_then_choice() -> chat::TestResult {
    cases::assert_rows("maestro_chat_usage_prefers_top_level_then_choice")
}

#[test]
fn maestro_message_usage_updates_only_reported_categories() -> chat::TestResult {
    cases::assert_rows("maestro_message_usage_updates_only_reported_categories")
}

#[test]
fn maestro_response_usage_preserves_reported_totals() -> chat::TestResult {
    cases::assert_rows("maestro_response_usage_preserves_reported_totals")
}

#[test]
fn maestro_conversation_usage_uses_total_or_fallback() -> chat::TestResult {
    cases::assert_rows("maestro_conversation_usage_uses_total_or_fallback")
}
