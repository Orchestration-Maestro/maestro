//! Session search behavior through the public chat interface.
#![cfg(test)]
#[path = "session_search_support/mod.rs"]
mod session_search_support;
use maestro_app::{Presence, SessionInfo};
use maestro_chat::*;

/// A populated owner record for direct borrowing witnesses.
fn session() -> SessionInfo {
    SessionInfo {
        path: "original".into(),
        id: Presence::Present(serde_json::json!("identity")),
        cwd: "directory".into(),
        name: Some("name".into()),
        parent_session_path: Presence::Missing,
        created: 1.0,
        modified: 2.0,
        message_count: 1,
        first_message: "preview".into(),
        all_messages_text: "alpha\n beta".into(),
    }
}

#[test]
fn regex_parse_reports_failure_or_compiled_pattern() {
    session_search_support::run_group("regex_parse_reports_failure_or_compiled_pattern");
}
#[test]
fn phrases_collapse_whitespace() {
    session_search_support::run_group("phrases_collapse_whitespace");
}
#[test]
fn relevance_uses_score_then_newest() {
    session_search_support::run_group("relevance_uses_score_then_newest");
}

#[test]
fn regex_matches_case_insensitive_words() {
    session_search_support::run_group("regex_matches_case_insensitive_words");
}

#[test]
fn recent_results_retain_incoming_order() {
    session_search_support::run_group("recent_results_retain_incoming_order");
}

#[test]
fn invalid_regex_has_no_results() {
    session_search_support::run_group("invalid_regex_has_no_results");
}

#[test]
fn all_names_keeps_unnamed_sessions() {
    session_search_support::run_group("all_names_keeps_unnamed_sessions");
}

#[test]
fn named_sessions_only() {
    session_search_support::run_group("named_sessions_only");
}

#[test]
fn names_are_filtered_before_matching() {
    session_search_support::run_group("names_are_filtered_before_matching");
}

#[test]
fn blank_names_are_not_named() {
    session_search_support::run_group("blank_names_are_not_named");
}

#[test]
fn query_whitespace_uses_shared_scalar_set() {
    session_search_support::run_group("query_whitespace_uses_shared_scalar_set");
}

#[test]
fn quotes_split_adjacent_fuzzy_and_phrase_terms() {
    session_search_support::run_group("quotes_split_adjacent_fuzzy_and_phrase_terms");
}

#[test]
fn unclosed_quotes_restore_plain_tokens() {
    session_search_support::run_group("unclosed_quotes_restore_plain_tokens");
}

#[test]
fn regex_prefix_is_lowercase_only() {
    session_search_support::run_group("regex_prefix_is_lowercase_only");
}

#[test]
fn names_use_shared_whitespace() {
    session_search_support::run_group("names_use_shared_whitespace");
}

#[test]
fn search_text_uses_four_fields() {
    session_search_support::run_group("search_text_uses_four_fields");
}

#[test]
fn search_text_ignores_other_metadata() {
    session_search_support::run_group("search_text_ignores_other_metadata");
}

#[test]
fn search_text_preserves_empty_field_separators() {
    session_search_support::run_group("search_text_preserves_empty_field_separators");
}

#[test]
fn phrase_search_normalizes_but_regex_keeps_raw_text() {
    session_search_support::run_group("phrase_search_normalizes_but_regex_keeps_raw_text");
}

#[test]
fn fuzzy_terms_delegate_to_toolkit() {
    session_search_support::run_group("fuzzy_terms_delegate_to_toolkit");
}

#[test]
fn tokens_require_all_matches_and_add_scores() {
    session_search_support::run_group("tokens_require_all_matches_and_add_scores");
}

#[test]
fn empty_query_matches_with_zero_score() {
    session_search_support::run_group("empty_query_matches_with_zero_score");
}

#[test]
fn blank_manual_phrase_adds_no_score() {
    session_search_support::run_group("blank_manual_phrase_adds_no_score");
}

#[test]
fn absent_regex_never_matches() {
    session_search_support::run_group("absent_regex_never_matches");
}

#[test]
fn regex_uses_ecmascript_patterns() {
    session_search_support::run_group("regex_uses_ecmascript_patterns");
}

#[test]
fn phrase_and_regex_scores_use_first_utf16_offset() {
    session_search_support::run_group("phrase_and_regex_scores_use_first_utf16_offset");
}

#[test]
fn threaded_search_uses_relevance_order() {
    session_search_support::run_group("threaded_search_uses_relevance_order");
}

#[test]
fn equal_ranks_keep_order_and_duplicates() {
    session_search_support::run_group("equal_ranks_keep_order_and_duplicates");
}

#[test]
fn empty_query_keeps_order_in_every_mode() {
    session_search_support::run_group("empty_query_keeps_order_in_every_mode");
}

#[test]
fn empty_input_has_no_results() {
    session_search_support::run_group("empty_input_has_no_results");
}

#[test]
fn omitted_name_filter_means_all() {
    session_search_support::run_group("omitted_name_filter_means_all");
}

#[test]
fn empty_quotes_are_not_an_empty_query() {
    session_search_support::run_group("empty_quotes_are_not_an_empty_query");
}

#[test]
fn malformed_ids_keep_other_searchable_fields() {
    session_search_support::run_group("malformed_ids_keep_other_searchable_fields");
}

#[test]
fn malformed_ids_do_not_invent_search_text() {
    session_search_support::run_group("malformed_ids_do_not_invent_search_text");
}

#[test]
fn results_borrow_original_sessions() {
    let records = [session(), session()];
    let selected =
        filter_and_sort_sessions(&records, "alpha", SortMode::Relevance, NameFilter::All);
    assert_eq!(selected.len(), 2);
    assert!(std::ptr::eq(selected[0], &raw const records[0]));
    assert!(std::ptr::eq(selected[1], &raw const records[1]));
}
