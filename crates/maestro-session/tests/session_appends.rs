//! Remaining append operations through the public history owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::support;
use maestro_models::UserContent;
use maestro_request::presence::Presence;
use maestro_session::SessionManager;
use maestro_session::session_manager::{
    BranchSummaryOptions, CompactionOptions, ReadonlySessionManager,
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::io;
use support::native::broken_history;
use support::session_history::keys;

/// An append under test, given the manager and an existing entry identity.
type Append = fn(&mut SessionManager, &str) -> io::Result<String>;

#[test]
fn remaining_appends_capture_parent_fields_and_leaf() {
    support::session_tree::run("remaining_appends_capture_parent_fields_and_leaf");
}

#[test]
fn optional_append_data_preserves_absence_null_and_open_values() {
    support::session_tree::run("optional_append_data_preserves_absence_null_and_open_values");
}

#[test]
fn summary_origins_keep_omitted_false_true_and_null() {
    support::session_tree::run("summary_origins_keep_omitted_false_true_and_null");
}

#[test]
fn custom_messages_retain_content_display_and_context() {
    support::session_tree::run("custom_messages_retain_content_display_and_context");
}

#[test]
fn remaining_appends_keep_accepted_memory_after_io_failure() {
    let cases: [(&str, Append); 7] = [
        ("thinking_level_change", |m, _| {
            m.append_thinking_level_change("high")
        }),
        ("model_change", |m, _| {
            m.append_model_change("provider", "model")
        }),
        ("compaction", |m, _| {
            m.append_compaction("recap", "cut", 3.0, CompactionOptions::default())
        }),
        ("custom", |m, _| {
            m.append_custom_entry("state", Presence::Null)
        }),
        ("session_info", |m, _| m.append_session_info("name")),
        ("custom_message", |m, _| {
            m.append_custom_message_entry(
                "note",
                UserContent::Text("text".into()),
                true,
                Presence::Missing,
            )
        }),
        ("branch_summary", |m, root| {
            m.branch_with_summary(Some(root), "recap", BranchSummaryOptions::default())
        }),
    ];
    for (kind, append) in cases {
        let scratch = Scratch::new().unwrap();
        let (mut manager, root) = broken_history(&scratch);
        let before = manager.get_entries().len();
        append(&mut manager, &root).unwrap_err();
        let entries = manager.get_entries();
        assert_eq!(entries.len(), before + 1, "{kind}");
        let accepted = entries[before];
        assert_eq!(accepted["type"], kind);
        assert_eq!(manager.get_leaf_id(), accepted["id"].as_str(), "{kind}");
        assert!(
            manager
                .get_entry(accepted["id"].as_str().unwrap())
                .is_some()
        );
    }
}

/// Append one record of every remaining kind below a published user/assistant pair.
fn append_every_kind(manager: &mut SessionManager, root: &str) {
    let details = Presence::Present(json!({"z": 1, "a": [1, 2, 1]}));
    manager.append_thinking_level_change("high").unwrap();
    manager.append_model_change("provider", "model").unwrap();
    let options = CompactionOptions {
        details: details.clone(),
        from_hook: Presence::Present(true),
        usage: Presence::Missing,
    };
    manager
        .append_compaction("recap", "absent", 17.0, options)
        .unwrap();
    manager
        .append_custom_entry("state", details.clone())
        .unwrap();
    manager.append_session_info(" \u{feff}named\t").unwrap();
    let content = UserContent::Text("shown".into());
    manager
        .append_custom_message_entry("note", content, false, details.clone())
        .unwrap();
    manager
        .append_label_change(root, Presence::Present("mark"))
        .unwrap();
    manager
        .branch_with_summary(
            None,
            "abandoned",
            BranchSummaryOptions {
                details,
                from_hook: Presence::Null,
                usage: Presence::Missing,
            },
        )
        .unwrap();
}

/// Member order of the first written record of a kind.
fn layout<'a>(lines: &'a [Value], kind: &str) -> Vec<&'a str> {
    keys(lines.iter().find(|line| line["type"] == kind).unwrap())
}

/// Fixed member order of every remaining record kind.
fn assert_wire_layouts(lines: &[Value]) {
    let base = ["id", "parentId", "timestamp"];
    assert_eq!(
        layout(lines, "custom"),
        ["type", "customType", "data", base[0], base[1], base[2]]
    );
    assert_eq!(
        layout(lines, "custom_message"),
        [
            "type",
            "customType",
            "content",
            "display",
            "details",
            "id",
            "parentId",
            "timestamp"
        ]
    );
    assert_eq!(
        layout(lines, "compaction"),
        [
            "type",
            "id",
            "parentId",
            "timestamp",
            "summary",
            "firstKeptEntryId",
            "tokensBefore",
            "details",
            "fromHook"
        ]
    );
    assert_eq!(
        layout(lines, "branch_summary"),
        [
            "type",
            "id",
            "parentId",
            "timestamp",
            "fromId",
            "summary",
            "details",
            "fromHook"
        ]
    );
    assert_eq!(
        layout(lines, "label"),
        ["type", "id", "parentId", "timestamp", "targetId", "label"]
    );
    assert_eq!(
        layout(lines, "session_info"),
        ["type", "id", "parentId", "timestamp", "name"]
    );
    assert_eq!(
        layout(lines, "model_change")[..4],
        ["type", "id", "parentId", "timestamp"]
    );
}

/// A reopened file yields the same ordered records, name, label and selection.
fn assert_reopened(manager: &SessionManager, path: &str, root: &str) {
    let reopened = SessionManager::open(path, None, None).unwrap();
    assert_eq!(reopened.get_session_name().unwrap(), Some("named"));
    assert_eq!(reopened.get_label(root), Some(&json!("mark")));
    assert_eq!(reopened.get_leaf_id(), manager.get_leaf_id());
    let (kept, loaded) = (manager.get_entries(), reopened.get_entries());
    assert_eq!(kept.len(), loaded.len());
    for (kept, loaded) in kept.iter().zip(loaded) {
        assert!(
            support::session_tree::same(kept, loaded),
            "{kept} != {loaded}"
        );
    }
}

#[test]
fn all_appends_round_trip_through_native_jsonl() {
    let scratch = Scratch::new().unwrap();
    let mut manager =
        SessionManager::create("/work", Some(scratch.path().to_str().unwrap()), "unused").unwrap();
    let root = manager
        .append_message(json!({"role": "user", "content": "root", "timestamp": 1}))
        .unwrap();
    manager
        .append_message(json!({"role": "assistant", "content": "reply", "timestamp": 2}))
        .unwrap();
    append_every_kind(&mut manager, &root);
    let path = manager.get_session_file().unwrap().to_owned();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.ends_with('\n') && !text.contains('\r'));
    assert!(text.contains(r#""tokensBefore":17,"#));
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_wire_layouts(&lines);
    assert_reopened(&manager, &path, &root);
}

/// The usage of a summary request.
fn summary_usage() -> maestro_request::types::Usage {
    serde_json::from_value(
        json!({"input": 12, "output": 3, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": 15, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
        "total": 0.25}}),
    )
    .unwrap()
}

#[test]
fn summary_records_keep_the_usage_that_generated_them() {
    let scratch = Scratch::new().unwrap();
    let dir = scratch.path().to_str().unwrap();
    let mut manager = SessionManager::create("/work", Some(dir), "unused").unwrap();
    let root = manager
        .append_message(json!({"role": "user", "content": "first", "timestamp": 1}))
        .unwrap();
    manager
        .append_message(json!({"role": "assistant", "content": "second", "timestamp": 2}))
        .unwrap();
    let usage = summary_usage();
    let compaction = manager
        .append_compaction(
            "recap",
            &root,
            3.0,
            CompactionOptions {
                usage: Presence::Present(usage.clone()),
                ..CompactionOptions::default()
            },
        )
        .unwrap();
    let branch = manager
        .branch_with_summary(
            Some(&root),
            "abandoned",
            BranchSummaryOptions {
                usage: Presence::Present(usage.clone()),
                ..BranchSummaryOptions::default()
            },
        )
        .unwrap();
    let plain = manager
        .append_compaction("bare", &root, 3.0, CompactionOptions::default())
        .unwrap();
    let expected = serde_json::to_value(&usage).unwrap();
    let written = std::fs::read_to_string(manager.get_session_file().unwrap()).unwrap();
    let stored = maestro_session::session_manager::parse_session_entries(&written);
    for id in [&compaction, &branch] {
        assert_eq!(manager.get_entry(id).unwrap()["usage"], expected);
        let reloaded = stored.iter().find(|entry| entry["id"] == id.as_str());
        assert_eq!(reloaded.unwrap()["usage"], expected);
    }
    assert!(manager.get_entry(&plain).unwrap().get("usage").is_none());
}
