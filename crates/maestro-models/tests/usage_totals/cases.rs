//! Closed ownership for recorded usage-accounting invocations.

use crate::{chat::TestResult, response_qualification::assert_corpus_rows};

/// Execute only the named test's recorded invocations.
pub(super) fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/usage_accounting.json"),
        &[
            (
                "crates/maestro-models/tests/usage_after_abort.rs::maestro_abort_retains_reported_usage",
                19,
            ),
            (
                "crates/maestro-models/tests/usage_after_abort.rs::maestro_abort_before_usage_keeps_zero_counters",
                6,
            ),
            (
                "crates/maestro-models/tests/usage_after_abort.rs::maestro_abort_keeps_each_reported_category",
                3,
            ),
            (
                "crates/maestro-models/tests/usage_after_abort.rs::maestro_final_only_usage_is_not_fabricated",
                8,
            ),
            (
                "crates/maestro-models/tests/usage_totals.rs::maestro_usage_preserves_provider_total",
                56,
            ),
            (
                "crates/maestro-models/tests/usage_totals.rs::maestro_chat_usage_avoids_cache_and_reasoning_double_count",
                9,
            ),
            (
                "crates/maestro-models/tests/usage_totals.rs::maestro_chat_usage_prefers_top_level_then_choice",
                4,
            ),
            (
                "crates/maestro-models/tests/usage_totals.rs::maestro_message_usage_updates_only_reported_categories",
                4,
            ),
            (
                "crates/maestro-models/tests/usage_totals.rs::maestro_response_usage_preserves_reported_totals",
                15,
            ),
            (
                "crates/maestro-models/tests/usage_totals.rs::maestro_conversation_usage_uses_total_or_fallback",
                4,
            ),
        ],
        test,
    )
}
