//! Corpus selection for controlled reasoning and text observations.

#![cfg(not(target_arch = "wasm32"))]
use crate::chat::TestResult;

/// Exact owning test paths and closed case counts.
const OWNERS: [(&str, usize); 15] = [
    (
        "crates/maestro-models/tests/cloud_model_routes.rs::maestro_cloud_catalog_keeps_configured_order",
        1,
    ),
    (
        "crates/maestro-models/tests/content_thinking_disabled.rs::maestro_shipped_models_disable_unrequested_thinking",
        4,
    ),
    (
        "crates/maestro-models/tests/conversation_reasoning.rs::maestro_conversation_off_omits_reasoning_controls",
        3,
    ),
    (
        "crates/maestro-models/tests/conversation_reasoning.rs::maestro_conversation_selects_native_reasoning_mode",
        15,
    ),
    (
        "crates/maestro-models/tests/conversation_tool_schema.rs::maestro_conversation_preserves_nested_schema_keywords",
        2,
    ),
    (
        "crates/maestro-models/tests/missing_tool_results.rs::maestro_missing_results_allow_follow_on_text",
        46,
    ),
    (
        "crates/maestro-models/tests/provider_unicode/inputs.rs::maestro_provider_preserves_emoji_tool_text",
        23,
    ),
    (
        "crates/maestro-models/tests/provider_unicode/inputs.rs::maestro_provider_preserves_mixed_comment_text",
        23,
    ),
    (
        "crates/maestro-models/tests/missing_tool_results.rs::maestro_missing_results_allow_another_tool_call",
        6,
    ),
    (
        "crates/maestro-models/tests/missing_tool_results.rs::maestro_pending_tools_flush_at_history_boundaries",
        12,
    ),
    (
        "crates/maestro-models/tests/missing_tool_results.rs::maestro_failed_assistants_do_not_leave_pending_tools",
        12,
    ),
    (
        "crates/maestro-models/tests/provider_unicode/inputs.rs::maestro_provider_preserves_valid_text_roles",
        6,
    ),
    (
        "crates/maestro-models/tests/provider_unicode/wire.rs::maestro_provider_decodes_split_utf8_scalars",
        6,
    ),
    (
        "crates/maestro-models/tests/provider_unicode/wire.rs::maestro_provider_replaces_invalid_utf8_bytes",
        6,
    ),
    (
        "crates/maestro-models/tests/provider_unicode/inputs.rs::maestro_sanitizer_retains_pairs_and_removes_lone_units",
        20,
    ),
];

/// Compare the owning test through the shared public-operation observations.
pub fn assert_rows(test: &str) -> TestResult {
    crate::response_qualification::assert_corpus_rows(
        include_str!("../fixtures/reasoning_qualification.json"),
        &OWNERS,
        test,
    )
}
