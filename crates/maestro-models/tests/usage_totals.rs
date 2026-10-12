//! Token totals and costs from controlled provider usage reports.

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

#[path = "usage_totals/cases.rs"]
mod cases;

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
