//! Compaction preparation through the public session interface.
#![cfg(test)]
/// Recorded operation support.
mod support {
    /// Compaction corpus consumer.
    pub mod compaction_cases;
    /// Complete numeric comparison.
    pub mod value;
}
#[test]
fn compaction_defaults_match_supplied_policy() {
    support::compaction_cases::group("defaults");
}
#[test]
fn context_usage_prefers_reported_total() {
    support::compaction_cases::group("usage_totals");
}

#[test]
fn latest_successful_usage_skips_failed_attempts() {
    support::compaction_cases::group("latest_usage");
}

#[test]
fn message_estimates_follow_role_content() {
    support::compaction_cases::group("estimates");
}

#[test]
fn context_estimate_adds_only_trailing_messages() {
    support::compaction_cases::group("context_estimate");
}

#[test]
fn compaction_trigger_is_strict_and_enabled() {
    support::compaction_cases::group("trigger");
}
#[test]
fn message_estimates_keep_utf16_lengths() {
    support::compaction_cases::group("scalar_estimates");
}

#[test]
fn turn_start_distinguishes_entry_and_message_roles() {
    support::compaction_cases::group("turn_starts");
}

#[test]
fn cut_threshold_selects_safe_message_boundary() {
    support::compaction_cases::group("cut_thresholds");
}

#[test]
fn cut_range_and_no_later_candidate_keep_fallback() {
    support::compaction_cases::group("cut_ranges");
}

#[test]
fn cut_absorbs_metadata_without_crossing_messages_or_compaction() {
    support::compaction_cases::group("cut_metadata");
}

#[test]
fn cut_start_preserves_empty_prefix_split_state() {
    support::compaction_cases::group("cut_turns");
}

#[test]
fn cut_candidates_cover_application_roles_and_ignore_metadata() {
    support::compaction_cases::group("cut_roles");
}

#[test]
fn file_tracking_selects_nonempty_paths_and_known_tools() {
    support::compaction_cases::group("file_extraction");
}

#[test]
fn file_lists_exclude_modified_reads_and_sort_paths() {
    support::compaction_cases::group("file_lists");
}

#[test]
fn preparation_keeps_prior_boundary_and_split_prefix() {
    support::compaction_cases::group("preparation");
}

#[test]
fn preparation_carries_only_builtin_prior_file_details() {
    support::compaction_cases::group("prior_files");
}

#[test]
fn preparation_tracks_history_and_prefix_not_kept_suffix() {
    support::compaction_cases::group("preparation_files");
}

#[test]
fn preparation_selects_latest_summary_and_boundary() {
    support::compaction_cases::group("preparation_boundaries");
}

#[test]
fn selected_reads_ignore_unconsumed_message_fields() {
    support::compaction_cases::group("selected_reads");
}

#[test]
fn native_selected_reads_return_errors_at_reached_fields() {
    support::compaction_cases::group("read_errors");
}

#[test]
fn preparation_leaves_branch_unchanged() {
    let entries = serde_json::json!([
        {"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"old","unknown":{"nested":true}}},
        {"type":"message","id":"b","parentId":"a","message":{"role":"user","content":"new"}}
    ]);
    let prepared = maestro_session::prepare_compaction(
        entries.as_array().unwrap(),
        &maestro_session::CompactionSettings {
            keep_recent_tokens: 1.0,
            ..maestro_session::DEFAULT_COMPACTION_SETTINGS
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(prepared.messages_to_summarize[0], entries[0]["message"]);
}

#[test]
fn large_history_preparation_preserves_fixture_structure() {
    support::compaction_cases::group("large_history");
}
