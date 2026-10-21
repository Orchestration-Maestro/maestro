//! Closed ownership for the recorded cache, overflow and tool-image invocations.

#![cfg(not(target_arch = "wasm32"))]
use crate::{chat::TestResult, response_qualification::assert_corpus_rows};

/// Execute only the named test's recorded invocations.
pub fn assert_rows(test: &str) -> TestResult {
    assert_corpus_rows(
        include_str!("../fixtures/cache_overflow_images.json"),
        &[
            (
                "crates/maestro-models/tests/provider_overflow/cases.rs::maestro_overflow_errors_keep_provider_routes",
                23,
            ),
            (
                "crates/maestro-models/tests/provider_overflow/cases.rs::maestro_successful_overflow_counts_cached_input",
                13,
            ),
            (
                "crates/maestro-models/tests/provider_overflow/cases.rs::maestro_filled_context_requires_zero_output",
                20,
            ),
            (
                "crates/maestro-models/tests/tool_images/cases.rs::maestro_image_results_continue_with_returned_calls",
                32,
            ),
            (
                "crates/maestro-models/tests/tool_images/cases.rs::maestro_mixed_results_preserve_text_and_images",
                32,
            ),
            (
                "crates/maestro-models/tests/cache_preferences.rs::maestro_cache_preferences_reach_each_protocol",
                111,
            ),
            (
                "crates/maestro-models/tests/cache_preferences.rs::maestro_cache_none_omits_affinity_key",
                42,
            ),
            (
                "crates/maestro-models/tests/cache_preferences.rs::maestro_cache_flags_keep_independent_markers",
                26,
            ),
            (
                "crates/maestro-models/tests/provider_overflow/cases.rs::maestro_throttling_is_not_context_overflow",
                10,
            ),
            (
                "crates/maestro-models/tests/provider_overflow/local_endpoints.rs::maestro_local_descriptors_keep_overflow_signals",
                4,
            ),
            (
                "crates/maestro-models/tests/tool_images/payloads.rs::maestro_tool_image_fallbacks_keep_protocol_text",
                120,
            ),
            (
                "crates/maestro-models/tests/tool_images/payloads.rs::maestro_tool_images_keep_block_order_and_duplicates",
                6,
            ),
        ],
        test,
    )
}
