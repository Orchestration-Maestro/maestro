//! Memory-history operations through the public owner.
#![cfg(test)]

use crate::support;
#[cfg(unix)]
use maestro_session::session_manager::ReadonlySessionManager;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn memory_session_starts_with_current_header() {
    if maestro_test_process::selection().is_child() {
        assert_ne!(chrono::Local::now().offset().local_minus_utc(), 0);
        support::session_history::group("memory_session_starts_with_current_header");
        return;
    }
    if !maestro_test_process::local_zone_follows_tz() {
        return;
    }
    let output =
        maestro_test_process::TestProcess::current("memory_session_starts_with_current_header")
            .unwrap()
            .nocapture()
            .case("*")
            .env("TZ", "America/New_York")
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "UTC emission child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn explicit_empty_cwd_is_not_defaulted() {
    support::session_history::group("explicit_empty_cwd_is_not_defaulted");
}

#[test]
fn missing_cwd_uses_the_process_directory() {
    support::session_history::group("missing_cwd_uses_the_process_directory");
}

#[test]
fn new_session_preserves_supplied_identity_and_parent() {
    support::session_history::group("new_session_preserves_supplied_identity_and_parent");
}

#[test]
fn generated_session_ids_are_v7_unique_and_ordered() {
    support::session_history::group("generated_session_ids_are_v7_unique_and_ordered");
}

#[test]
fn message_append_publishes_parent_index_and_leaf() {
    support::session_history::group("message_append_publishes_parent_index_and_leaf");
}

#[test]
fn branch_reads_root_to_selected_entry() {
    support::session_history::group("branch_reads_root_to_selected_entry");
}

#[test]
fn missing_entries_and_empty_branches_are_absent() {
    support::session_history::group("missing_entries_and_empty_branches_are_absent");
}

#[test]
fn entry_reads_select_the_requested_identity() {
    support::session_history::group("entry_reads_select_the_requested_identity");
}

#[test]
fn branching_keeps_abandoned_history_and_uses_selected_parent() {
    support::session_history::group("branching_keeps_abandoned_history_and_uses_selected_parent");
}

#[test]
fn unknown_branch_errors_without_changing_selection() {
    support::session_history::group("unknown_branch_errors_without_changing_selection");
}

#[test]
fn selecting_current_leaf_keeps_history() {
    support::session_history::group("selecting_current_leaf_keeps_history");
}

#[test]
fn explicit_empty_branch_does_not_select_current_leaf() {
    support::session_history::group("explicit_empty_branch_does_not_select_current_leaf");
}

#[test]
fn new_session_discards_active_history_and_resets_parent() {
    support::session_history::group("new_session_discards_active_history_and_resets_parent");
}

#[test]
fn append_keeps_open_message_values_without_decoding() {
    support::session_history::group("append_keeps_open_message_values_without_decoding");
}

/// Native cwd observation from the shared corpus.
#[cfg(unix)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeCase {
    /// Diagnostic label.
    id: String,
    /// Owning test.
    test: String,
    /// Invoked native operation.
    operation: String,
    /// Full expected native result.
    expected: serde_json::Value,
}

#[test]
#[cfg(unix)]
fn explicit_cwd_survives_a_missing_process_directory() {
    if maestro_test_process::selection().is_child() {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/session_history.json")).unwrap();
        let selected: Vec<NativeCase> = rows
            .into_iter()
            .filter(|row| row["operation"] == "native_cwd")
            .map(|row| serde_json::from_value(row).unwrap())
            .collect();
        assert_eq!(selected.len(), 1);
        let case = &selected[0];
        assert_eq!(case.id, "deleted-cwd");
        assert_eq!(
            case.test,
            "explicit_cwd_survives_a_missing_process_directory"
        );
        assert_eq!(case.operation, "native_cwd");
        let original = std::env::current_dir().unwrap();
        let removed = maestro_test_directory::Scratch::new().unwrap();
        removed.enter_removed("removed").unwrap();
        let explicit = maestro_session::SessionManager::in_memory(Some("")).unwrap();
        let native = std::env::current_dir().unwrap_err();
        let error = maestro_session::SessionManager::in_memory(None)
            .err()
            .unwrap();
        std::env::set_current_dir(original).unwrap();
        assert_eq!(error.raw_os_error(), native.raw_os_error());
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(
            serde_json::json!({"explicitEmpty": explicit.get_cwd(), "defaultFails": true}),
            case.expected
        );
        return;
    }
    let output = maestro_test_process::TestProcess::current(
        "explicit_cwd_survives_a_missing_process_directory",
    )
    .unwrap()
    .nocapture()
    .case("*")
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "cwd child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
