//! Recursive attachment completion through controlled host operations.
#![cfg(test)]
/// Shared host observations.
pub mod fixtures {
    pub mod attachments;
    pub mod futures;
    pub mod pending_attachments;
}
use fixtures::attachments::run_cases;
use fixtures::pending_attachments::{exported_provider, observe_pending};

#[cfg(not(windows))]
#[test]
fn attachment_routing_precedes_commands_and_forced_paths() {
    run_cases("attachment_routing_precedes_commands_and_forced_paths");
}

#[cfg(not(windows))]
#[test]
fn scoped_search_retains_authored_directory_spelling() {
    run_cases("scoped_search_retains_authored_directory_spelling");
}

#[cfg(not(windows))]
#[test]
fn scoped_home_failure_stops_search_without_fallback() {
    run_cases("scoped_home_failure_stops_search_without_fallback");
}

#[cfg(not(windows))]
#[test]
fn fd_queries_preserve_literal_case_and_separator_matching() {
    run_cases("fd_queries_preserve_literal_case_and_separator_matching");
}

#[cfg(not(windows))]
#[test]
fn attachment_scores_sort_stably_without_deduplication() {
    run_cases("attachment_scores_sort_stably_without_deduplication");
}

#[cfg(not(windows))]
#[test]
fn empty_attachment_queries_keep_process_order() {
    run_cases("empty_attachment_queries_keep_process_order");
}

#[cfg(not(windows))]
#[test]
fn attachment_scores_use_whole_string_lowercase() {
    run_cases("attachment_scores_use_whole_string_lowercase");
}

#[cfg(not(windows))]
#[test]
fn attachment_search_limits_results_after_stable_ranking() {
    run_cases("attachment_search_limits_results_after_stable_ranking");
}

#[cfg(not(windows))]
#[test]
fn fd_output_keeps_filename_identity_and_git_components() {
    run_cases("fd_output_keeps_filename_identity_and_git_components");
}

#[cfg(not(windows))]
#[test]
fn process_failures_and_cancellation_discard_partial_output() {
    run_cases("process_failures_and_cancellation_discard_partial_output");
}

#[cfg(not(windows))]
#[test]
fn selected_attachment_roundtrips_through_insertion() {
    run_cases("selected_attachment_roundtrips_through_insertion");
}

#[cfg(windows)]
#[test]
fn windows_attachment_paths_keep_devices_and_separators() {
    run_cases("windows_attachment_paths_keep_devices_and_separators");
}

#[test]
fn completion_exports_remain_usable_from_crate_root() {
    use fixtures::futures::block_on;
    use maestro_tui::AutocompleteProvider;
    use maestro_tui::autocomplete::{CompletionOptions, CursorPosition};
    use std::cell::Cell;
    let signal = Cell::new(false);
    let provider = exported_provider();
    let lines = ["/".into()];
    let cursor = CursorPosition { line: 0, col: 1 };
    let result = block_on(provider.get_suggestions(
        &lines,
        cursor,
        CompletionOptions {
            signal: &signal,
            force: None,
        },
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        result
            .items
            .iter()
            .map(|i| i.value.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    let applied = provider.apply_completion(&lines, cursor, &result.items[1], &result.prefix);
    assert_eq!(
        (applied.lines, applied.cursor_col),
        (vec!["/beta ".to_owned()], 6)
    );
    assert_eq!(
        provider.should_trigger_file_completion(&lines, cursor),
        Some(false)
    );
    signal.set(true);
    assert!(
        block_on(provider.get_suggestions(
            &["@".into()],
            cursor,
            CompletionOptions {
                signal: &signal,
                force: None
            }
        ))
        .unwrap()
        .is_none()
    );
}

#[test]
fn replaceable_attachment_operations_preserve_provider_policy() {
    assert_eq!(observe_pending(false), observe_pending(true));
}
