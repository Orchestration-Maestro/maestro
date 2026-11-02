//! Compaction preparation through the public session interface.
#![cfg(test)]
use crate::support;
#[test]
fn compaction_defaults_match_supplied_policy() {
    support::compaction_cases::group("defaults");
}
#[test]
fn context_usage_prefers_reported_total() {
    support::compaction_cases::group("usage_totals");
}

/// A saved usage whose categories add up to 13 and whose total is given, or omitted.
fn saved_usage(total: Option<serde_json::Value>) -> serde_json::Value {
    let mut usage = serde_json::json!({
        "input": 7.0,
        "output": 3.0,
        "cacheRead": 2.0,
        "cacheWrite": 1.0,
        "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 1.0}
    });
    if let Some(total) = total {
        usage["totalTokens"] = total;
    }
    usage
}

/// Re-read a saved usage through the typed record and its written form.
fn roundtrip(total: Option<serde_json::Value>) -> serde_json::Value {
    let saved = saved_usage(total);
    let usage: maestro_request::types::Usage = serde_json::from_value(saved.clone()).unwrap();
    let written = serde_json::to_value(usage).unwrap();
    assert_eq!(written.get("totalTokens"), saved.get("totalTokens"));
    written
}

#[test]
fn context_totals_keep_fallback_after_record_roundtrip() {
    use serde_json::json;

    for total in [None, Some(json!(null)), Some(json!(0.0)), Some(json!(-0.0))] {
        let written = roundtrip(total.clone());
        assert_eq!(
            maestro_session::calculate_context_tokens(&written)
                .unwrap()
                .to_bits(),
            13.0_f64.to_bits(),
            "{total:?}"
        );
    }
    for (total, expected) in [
        (json!(97.0), 97.0_f64),
        (json!(1.5), 1.5),
        (json!(-2.0), -2.0),
    ] {
        assert_eq!(
            maestro_session::calculate_context_tokens(&roundtrip(Some(total)))
                .unwrap()
                .to_bits(),
            expected.to_bits()
        );
    }
    let mut zeroed = saved_usage(None);
    for member in ["input", "output", "cacheRead", "cacheWrite"] {
        zeroed[member] = json!(0.0);
    }
    let usage: maestro_request::types::Usage = serde_json::from_value(zeroed).unwrap();
    let written = serde_json::to_value(usage).unwrap();
    assert_eq!(
        maestro_session::calculate_context_tokens(&written)
            .unwrap()
            .to_bits(),
        0.0_f64.to_bits()
    );
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
