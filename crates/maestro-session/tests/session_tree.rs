//! Labels, names, resets and tree navigation through the public history owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::support;
use chrono::TimeZone;
use maestro_request::presence::Presence;
use maestro_session::SessionManager;
use maestro_session::session_manager::{ReadonlySessionManager, SessionTreeNode};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use support::native::broken_history;

/// Write a native history file below the scratch directory.
fn history_file(scratch: &Scratch, name: &str, records: &[Value]) -> String {
    let mut text = String::from(
        r#"{"type":"session","version":3,"id":"stored","cwd":"/fixture","timestamp":"2025-01-01T00:00:00.000Z"}"#,
    );
    for record in records {
        text.push('\n');
        text.push_str(&record.to_string());
    }
    let path = scratch.path().join(name);
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_owned()
}

/// A custom record with explicit identity, parent and instant.
fn record(id: &str, parent: Option<&str>, timestamp: &str) -> Value {
    json!({"type": "custom", "customType": "fixture", "id": id, "parentId": parent, "timestamp": timestamp})
}

/// A label record for a target.
fn label(id: &str, parent: &str, target: &str, text: &str) -> Value {
    json!({"type": "label", "id": id, "parentId": parent, "timestamp": "2025-01-02T00:00:00.000Z", "targetId": target, "label": text})
}

/// Compact `id(child,child)` rendering of tree shape.
fn shape(node: &SessionTreeNode<'_>) -> String {
    let children: Vec<_> = node.children.iter().map(shape).collect();
    let id = node.entry["id"].as_str().unwrap();
    if children.is_empty() {
        id.to_owned()
    } else {
        format!("{id}({})", children.join(","))
    }
}

#[test]
fn label_changes_set_clear_and_keep_current_parent() {
    support::session_tree::run("label_changes_set_clear_and_keep_current_parent");
}

#[test]
fn latest_label_and_timestamp_reach_tree_nodes() {
    support::session_tree::run("latest_label_and_timestamp_reach_tree_nodes");
}

#[test]
fn unknown_targets_fail_before_any_mutation() {
    support::session_tree::run("unknown_targets_fail_before_any_mutation");
}

#[test]
fn session_names_use_exact_whitespace_and_latest_entry() {
    support::session_tree::run("session_names_use_exact_whitespace_and_latest_entry");
}

#[test]
fn names_are_session_wide_and_latest_clear_wins() {
    support::session_tree::run("names_are_session_wide_and_latest_clear_wins");
}

#[test]
fn loaded_names_decode_only_the_latest_selected_name() {
    support::session_tree::run("loaded_names_decode_only_the_latest_selected_name");
}

#[test]
fn newer_names_bypass_older_malformed_names() {
    support::session_tree::run("newer_names_bypass_older_malformed_names");
}

#[test]
fn empty_tree_branch_and_unknown_reads_are_absent() {
    support::session_tree::run("empty_tree_branch_and_unknown_reads_are_absent");
}

#[test]
fn custom_state_and_labels_stay_out_of_model_context() {
    support::session_tree::run("custom_state_and_labels_stay_out_of_model_context");
}

#[test]
fn tree_keeps_linear_sibling_and_nested_branches() {
    support::session_tree::run("tree_keeps_linear_sibling_and_nested_branches");
}

#[test]
fn reset_leaf_retains_history_and_next_append_starts_root() {
    support::session_tree::run("reset_leaf_retains_history_and_next_append_starts_root");
}

#[test]
fn reset_leaf_selects_before_first_without_writing() {
    support::session_tree::run("reset_leaf_selects_before_first_without_writing");
}

#[test]
fn reset_leaf_leaves_native_file_bytes_unchanged() {
    let scratch = Scratch::new().unwrap();
    let mut manager =
        SessionManager::create("/work", Some(scratch.path().to_str().unwrap()), "unused").unwrap();
    manager
        .append_message(json!({"role": "user", "content": "first", "timestamp": 1}))
        .unwrap();
    manager
        .append_message(json!({"role": "assistant", "content": "second", "timestamp": 2}))
        .unwrap();
    let file = manager.get_session_file().unwrap().to_owned();
    let before = std::fs::read(&file).unwrap();
    manager.reset_leaf();
    assert_eq!(manager.get_leaf_id(), None);
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(manager.get_entries().len(), 2);
}

#[test]
fn summary_branch_uses_selected_point_or_literal_root() {
    support::session_tree::run("summary_branch_uses_selected_point_or_literal_root");
}

#[test]
fn empty_summary_is_stored_but_not_projected() {
    support::session_tree::run("empty_summary_is_stored_but_not_projected");
}

#[test]
fn tree_roots_cover_null_self_missing_and_orphan_parents() {
    support::session_tree::run("tree_roots_cover_null_self_missing_and_orphan_parents");
}

#[test]
fn children_use_first_identity_order_and_last_record_values() {
    support::session_tree::run("children_use_first_identity_order_and_last_record_values");
}

#[test]
fn tree_duplicates_keep_edges_and_last_record_observations() {
    support::session_tree::run("tree_duplicates_keep_edges_and_last_record_observations");
}

#[test]
fn tree_children_sort_chronologically_with_stable_millisecond_ties() {
    support::session_tree::run("tree_children_sort_chronologically_with_stable_millisecond_ties");
}

#[test]
fn tree_timestamp_order_reuses_iso_and_offset_parsing() {
    support::session_tree::run("tree_timestamp_order_reuses_iso_and_offset_parsing");
}

#[test]
fn invalid_tree_timestamps_follow_valid_ones_stably() {
    support::session_tree::run("invalid_tree_timestamps_follow_valid_ones_stably");
}

#[test]
fn rootless_cycles_remain_absent() {
    support::session_tree::run("rootless_cycles_remain_absent");
}

#[test]
fn decoded_entry_identities_match_without_display_conversion() {
    support::session_tree::run("decoded_entry_identities_match_without_display_conversion");
}

#[test]
fn loaded_labels_keep_raw_values_until_consumed() {
    support::session_tree::run("loaded_labels_keep_raw_values_until_consumed");
}

#[test]
fn loading_retains_labels_for_targets_not_yet_present() {
    support::session_tree::run("loading_retains_labels_for_targets_not_yet_present");
}

#[test]
fn replacing_loaded_label_with_missing_timestamp_removes_old_time() {
    support::session_tree::run("replacing_loaded_label_with_missing_timestamp_removes_old_time");
}

#[test]
fn loaded_label_timestamps_preserve_explicit_null() {
    support::session_tree::run("loaded_label_timestamps_preserve_explicit_null");
}

#[test]
fn loaded_label_targets_use_string_identity_without_coercion() {
    support::session_tree::run("loaded_label_targets_use_string_identity_without_coercion");
}

#[test]
fn duplicate_fields_keep_last_values_and_decoded_identity() {
    support::session_tree::run("duplicate_fields_keep_last_values_and_decoded_identity");
}

#[test]
fn readonly_view_exposes_thirteen_current_reads() {
    support::session_tree::run("readonly_view_exposes_thirteen_current_reads");
    let mut manager = SessionManager::in_memory(Some("/view")).unwrap();
    let first = manager
        .append_message(json!({"role": "user", "content": "one", "timestamp": 1}))
        .unwrap();
    {
        let view: &dyn ReadonlySessionManager = &manager;
        assert_eq!(view.get_leaf_id(), Some(first.as_str()));
        assert_eq!(view.get_session_name().unwrap(), None);
    }
    manager.append_session_info("later").unwrap();
    manager
        .append_label_change(&first, Presence::Present("tag"))
        .unwrap();
    let view: &dyn ReadonlySessionManager = &manager;
    assert_eq!(view.get_session_name().unwrap(), Some("later"));
    assert_eq!(view.get_label(&first), Some(&json!("tag")));
    assert_eq!(view.get_entries().len(), 3);
}

#[test]
fn deep_trees_build_and_drop_without_recursive_stack_growth() {
    const DEPTH: usize = 30_000;
    let scratch = Scratch::new().unwrap();
    let records: Vec<Value> = (0..DEPTH)
        .map(|index| {
            let parent = index.checked_sub(1).map(|parent| format!("n{parent}"));
            record(
                &format!("n{index}"),
                parent.as_deref(),
                "2025-01-01T00:00:00.000Z",
            )
        })
        .collect();
    let path = history_file(&scratch, "deep.jsonl", &records);
    let manager = SessionManager::open(&path, None, None).unwrap();
    let tree = manager.get_tree();
    assert_eq!(tree.len(), 1);
    let (mut node, mut count) = (&tree[0], 1);
    while let Some(child) = node.children.first() {
        assert_eq!(node.children.len(), 1);
        node = child;
        count += 1;
    }
    assert_eq!(count, DEPTH);
    assert_eq!(node.entry["id"], format!("n{}", DEPTH - 1));
}

#[test]
fn tree_local_dates_use_the_native_local_zone() {
    let name = "tree_local_dates_use_the_native_local_zone";
    let zone = std::env::var("MAESTRO_TREE_ZONE");
    if cfg!(windows) || zone.is_ok() {
        let scratch = Scratch::new().unwrap();
        let path = history_file(
            &scratch,
            "local.jsonl",
            &[
                record("root", None, "2025-01-01T00:00:00.000Z"),
                record("local", Some("root"), "2025-01-01T01:00:00"),
                record("utc", Some("root"), "2025-01-01T03:00:00.000Z"),
                record("date", Some("root"), "2025-01-01"),
            ],
        );
        let manager = SessionManager::open(&path, None, None).unwrap();
        let tree = manager.get_tree();
        let local = chrono::Local
            .with_ymd_and_hms(2025, 1, 1, 1, 0, 0)
            .earliest()
            .unwrap()
            .timestamp_millis();
        if zone.as_deref() == Ok("America/New_York") {
            assert_ne!(chrono::Local::now().offset().local_minus_utc(), 0);
        }
        let utc_three = 1_735_700_400_000;
        let expected = if local < utc_three {
            "root(date,local,utc)"
        } else {
            "root(date,utc,local)"
        };
        assert_eq!(shape(&tree[0]), expected);
        return;
    }
    if !maestro_test_process::local_zone_follows_tz() {
        return;
    }
    for zone in ["UTC", "America/New_York"] {
        let output = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .nocapture()
            .env("TZ", zone)
            .env("MAESTRO_TREE_ZONE", zone)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{zone}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn failed_label_publication_keeps_old_resolved_maps() {
    for next in [
        Presence::Present("new"),
        Presence::Present(""),
        Presence::Missing,
        Presence::Null,
    ] {
        let scratch = Scratch::new().unwrap();
        let seeded = "2000-01-01T00:00:00.000Z";
        let file = history_file(
            &scratch,
            "labels.jsonl",
            &[
                json!({"type": "message", "id": "first", "parentId": null, "timestamp": seeded,
                    "message": {"role": "user", "content": "first", "timestamp": 1}}),
                json!({"type": "message", "id": "reply", "parentId": "first", "timestamp": seeded,
                    "message": {"role": "assistant", "content": "second", "timestamp": 2}}),
                json!({"type": "label", "id": "old", "parentId": "reply", "timestamp": seeded,
                    "targetId": "first", "label": "old"}),
            ],
        );
        let mut manager = SessionManager::open(&file, None, None).unwrap();
        let first = "first".to_owned();
        assert_eq!(manager.get_tree()[0].label_timestamp, Some(&json!(seeded)));
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        let before = manager.get_entries().len();
        manager.append_label_change(&first, next).unwrap_err();
        assert_eq!(manager.get_entries().len(), before + 1);
        let leaf = manager.get_leaf_entry().unwrap();
        assert_eq!(leaf["type"], "label");
        assert_ne!(leaf["timestamp"], json!(seeded));
        assert_eq!(manager.get_label(&first), Some(&json!("old")));
        let tree = manager.get_tree();
        assert_eq!(tree[0].label, Some(&json!("old")));
        assert_eq!(tree[0].label_timestamp, Some(&json!(seeded)));
    }
    let scratch = Scratch::new().unwrap();
    let (mut manager, first) = broken_history(&scratch);
    manager
        .append_label_change(&first, Presence::Present("fresh"))
        .unwrap_err();
    assert_eq!(manager.get_label(&first), None);
}

#[test]
fn switching_histories_rebuilds_labels_with_candidate_state() {
    let scratch = Scratch::new().unwrap();
    let labeled = history_file(
        &scratch,
        "labeled.jsonl",
        &[
            record("t", None, "2025-01-01T00:00:00Z"),
            label("l", "t", "t", "first"),
        ],
    );
    let other = history_file(
        &scratch,
        "other.jsonl",
        &[
            record("u", None, "2025-01-01T00:00:00Z"),
            label("m", "u", "u", "second"),
            label("n", "m", "t", "stale"),
        ],
    );
    let cleared = history_file(
        &scratch,
        "cleared.jsonl",
        &[
            record("t", None, "2025-01-01T00:00:00Z"),
            label("l", "t", "t", "first"),
            label("c", "l", "t", ""),
        ],
    );
    let mut manager = SessionManager::open(&labeled, None, None).unwrap();
    assert_eq!(manager.get_label("t"), Some(&json!("first")));
    manager.branch("t").unwrap();
    assert_eq!(manager.get_label("t"), Some(&json!("first")));
    manager.set_session_file(&other).unwrap();
    assert_eq!(manager.get_label("u"), Some(&json!("second")));
    assert_eq!(manager.get_label("t"), Some(&json!("stale")));
    manager.set_session_file(&labeled).unwrap();
    assert_eq!(manager.get_label("u"), None);
    assert_eq!(manager.get_label("t"), Some(&json!("first")));
    manager.set_session_file(&cleared).unwrap();
    assert_eq!(manager.get_label("t"), None);
    assert_eq!(manager.get_tree()[0].label_timestamp, None);
}

#[test]
fn failed_history_switch_preserves_existing_labels() {
    let scratch = Scratch::new().unwrap();
    let labeled = history_file(
        &scratch,
        "labeled.jsonl",
        &[
            record("t", None, "2025-01-01T00:00:00Z"),
            label("l", "t", "t", "kept"),
        ],
    );
    let index_failure = history_file(
        &scratch,
        "failing.jsonl",
        &[
            label("x", "t", "u", "intruder"),
            json!({"type": "message", "id": null}),
        ],
    );
    let unreadable = scratch.path().to_str().unwrap().to_owned();
    let rewrite_failure = history_file(&scratch, "headerless.jsonl", &[]);
    std::fs::write(&rewrite_failure, "not a session header\n").unwrap();
    let mut permissions = std::fs::metadata(&rewrite_failure).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&rewrite_failure, permissions).unwrap();
    // A privileged Unix run rewrites the read-only file anyway, so there is nothing to check.
    #[cfg(unix)]
    if maestro_test_directory::deny(&rewrite_failure, 0o444)
        .unwrap()
        .is_none()
    {
        return;
    }
    let mut manager = SessionManager::open(&labeled, None, None).unwrap();
    let stamp = manager.get_tree()[0].label_timestamp.cloned();
    for failing in [&index_failure, &unreadable, &rewrite_failure] {
        manager.set_session_file(failing).unwrap_err();
        assert_eq!(manager.get_label("t"), Some(&json!("kept")));
        assert_eq!(manager.get_label("u"), None);
        assert_eq!(manager.get_tree()[0].label_timestamp.cloned(), stamp);
        assert_eq!(manager.get_entries().len(), 2);
        assert_eq!(manager.get_session_file(), Some(labeled.as_str()));
    }
}

#[test]
fn tree_containers_are_fresh_over_borrowed_entries() {
    let scratch = Scratch::new().unwrap();
    let path = history_file(
        &scratch,
        "tree.jsonl",
        &[
            record("a", None, "2025-01-01T00:00:00Z"),
            record("b", Some("a"), "2025-01-01T00:00:01Z"),
            record("c", Some("a"), "2025-01-01T00:00:02Z"),
        ],
    );
    let manager = SessionManager::open(&path, None, None).unwrap();
    let mut first = manager.get_tree();
    let second = manager.get_tree();
    first[0].children.clear();
    assert_eq!(shape(&first[0]), "a");
    assert_eq!(shape(&second[0]), "a(b,c)");
    assert_eq!(shape(&manager.get_tree()[0]), "a(b,c)");
    assert!(std::ptr::eq(
        second[0].children[1].entry,
        manager.get_entry("c").unwrap()
    ));
}

#[test]
fn reachable_tree_cycles_omit_only_active_path_back_edges() {
    let scratch = Scratch::new().unwrap();
    let stamp = |second: u32| format!("2025-01-01T00:00:{second:02}Z");
    let path = history_file(
        &scratch,
        "cycle.jsonl",
        &[
            record("r", None, &stamp(1)),
            record("a", Some("r"), &stamp(2)),
            record("b", Some("r"), &stamp(3)),
            record("c", Some("a"), &stamp(4)),
            record("x", Some("b"), &stamp(5)),
            record("x", Some("c"), &stamp(6)),
            record("r", Some("a"), &stamp(7)),
        ],
    );
    let manager = SessionManager::open(&path, None, None).unwrap();
    let tree = manager.get_tree();
    assert_eq!(tree.len(), 1);
    assert_eq!(shape(&tree[0]), "r(a(c(x)),b(x))");
    assert_eq!(tree[0].entry["parentId"], "a");
    assert_eq!(manager.get_children("a").len(), 2);
}

#[test]
fn forest_constructor_uses_each_callers_selected_metadata() {
    use maestro_session::export_html::template::build_tree;
    let scratch = Scratch::new().unwrap();
    let entries = [
        record("t", None, "2025-01-01T00:00:00Z"),
        label("l", "t", "t", "kept"),
        label("c", "l", "t", ""),
    ];
    let file = history_file(&scratch, "policies.jsonl", &entries);
    let manager = SessionManager::open(&file, None, None).unwrap();
    let viewer = build_tree(
        manager.get_entries(),
        |id| (id == "t").then_some(&entries[1]["label"]),
        |_| None,
    );
    let cleared = manager.get_tree();
    assert_eq!(shape(&viewer[0]), shape(&cleared[0]));
    assert_eq!(viewer[0].label, Some(&json!("kept")));
    assert_eq!(viewer[0].label_timestamp, None);
    assert_eq!(cleared[0].label, None);
    assert_eq!(cleared[0].label_timestamp, None);
    assert!(std::ptr::eq(
        viewer[0].entry,
        manager.get_entry("t").unwrap()
    ));
    let selected = build_tree(
        manager.get_entries(),
        |_| Some(&entries[1]["label"]),
        |_| Some(&entries[1]["timestamp"]),
    );
    assert_eq!(selected[0].label_timestamp, Some(&entries[1]["timestamp"]));
    assert!(std::ptr::eq(selected[0].entry, viewer[0].entry));
}
