//! Branch extraction and full-file forking through the public history owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::support;
use maestro_request::presence::Presence;
use maestro_session::SessionManager;
use maestro_session::session_manager::{ReadonlySessionManager, parse_session_entries};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::fs;
use support::native::{below, below_keeping_dots, broken_history, uncanonicalized_scratch};

/// Message record with a fixed timestamp.
fn message(id: &str, parent: Option<&str>, role: &str) -> Value {
    json!({"type": "message", "id": id, "parentId": parent, "timestamp": "2025-01-01T00:00:00.000Z",
        "message": {"role": role, "content": id, "timestamp": 1}})
}

/// Label record resolving `label` for `target`.
fn label(
    id: &str,
    parent: Option<&str>,
    target: &str,
    label: impl Into<Value>,
    time: &str,
) -> Value {
    json!({"type": "label", "id": id, "parentId": parent, "timestamp": time,
        "targetId": target, "label": label.into()})
}

/// Write a history file below a fixed header and open it natively.
fn open(scratch: &Scratch, name: &str, records: &[Value]) -> SessionManager {
    let path = write(scratch, name, records);
    SessionManager::open(&path, Some(scratch.path().to_str().unwrap()), Some("/work")).unwrap()
}

/// Write a history file under the given header and return its path.
fn legacy(scratch: &Scratch, name: &str, header: &Value, lines: &[Value]) -> String {
    let text: String = std::iter::once(header)
        .chain(lines)
        .map(|record| record.to_string() + "\n")
        .collect();
    let path = below(scratch.path(), name);
    fs::write(&path, text).unwrap();
    path
}

/// Write a version-3 history file and return its path.
fn write(scratch: &Scratch, name: &str, records: &[Value]) -> String {
    let header = json!({"type": "session", "version": 3, "id": "source", "cwd": "/src",
        "timestamp": "2025-01-01T00:00:00.000Z"});
    legacy(scratch, name, &header, records)
}

/// Identities of the retained entries, in order.
fn ids(manager: &SessionManager) -> Vec<String> {
    manager
        .get_entries()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Records of a published file, header first.
fn records(path: &str) -> Vec<Value> {
    parse_session_entries(&fs::read_to_string(path).unwrap())
}

/// A fresh time-ordered UUID is stored as the session identity.
fn assert_v7(id: &str) {
    assert_eq!(uuid::Uuid::parse_str(id).unwrap().get_version_num(), 7);
}

/// Fresh memory history without a filename keeps none after extracting a middle entry.
fn extract_middle_of_fresh_memory() {
    let mut fresh = SessionManager::in_memory(Some("/work")).unwrap();
    let first = fresh
        .append_message(json!({"role": "user", "content": "a", "timestamp": 1}))
        .unwrap();
    let middle = fresh
        .append_message(json!({"role": "assistant", "content": "b", "timestamp": 2}))
        .unwrap();
    let last = fresh
        .append_message(json!({"role": "user", "content": "c", "timestamp": 3}))
        .unwrap();
    assert_eq!(fresh.get_session_file(), None);
    assert_eq!(fresh.create_branched_session(&middle).unwrap(), None);
    assert_eq!(ids(&fresh), [first, middle.clone()]);
    assert_eq!(fresh.get_entry(&last), None);
    assert_eq!(fresh.get_leaf_id(), Some(middle.as_str()));
    assert_eq!(fresh.get_session_file(), None);
}

#[test]
fn extraction_selects_only_the_requested_ancestry() {
    let scratch = Scratch::new().unwrap();
    let mut manager = SessionManager::in_memory(Some("/work")).unwrap();
    manager
        .set_session_file(&below(scratch.path(), "absent.jsonl"))
        .unwrap();
    let a = manager
        .append_message(json!({"role": "user", "content": "a", "timestamp": 1}))
        .unwrap();
    let b = manager
        .append_message(json!({"role": "assistant", "content": "b", "timestamp": 2}))
        .unwrap();
    let off = manager
        .append_message(json!({"role": "user", "content": "off", "timestamp": 3}))
        .unwrap();
    manager.branch(&b).unwrap();
    let d = manager
        .append_message(json!({"role": "user", "content": "d", "timestamp": 4}))
        .unwrap();
    let file = manager.get_session_file().map(str::to_owned);
    assert!(file.is_some());
    let old_id = manager.get_session_id().to_owned();
    assert_eq!(manager.create_branched_session(&d).unwrap(), None);
    assert_eq!(ids(&manager), [a, b.clone(), d.clone()]);
    assert_eq!(manager.get_entry(&off), None);
    assert_eq!(manager.get_leaf_id(), Some(d.as_str()));
    assert_eq!(manager.get_session_file().map(str::to_owned), file);
    assert_ne!(manager.get_session_id(), old_id);
    assert_v7(manager.get_session_id());
    let header = manager.get_header().unwrap();
    assert_eq!(header["cwd"], "/work");
    assert_eq!(header["id"], manager.get_session_id());
    assert!(header.get("parentSession").is_none());
    manager.branch(&b).unwrap();
    assert_eq!(manager.get_branch(None).unwrap().len(), 2);
    extract_middle_of_fresh_memory();
}

#[test]
fn unknown_extraction_leaf_preserves_the_owner() {
    let scratch = Scratch::new().unwrap();
    let blank = message("", None, "user");
    let mut native = open(&scratch, "blank.jsonl", &[blank]);
    assert!(native.get_branch(Some("")).unwrap().is_empty());
    let file = native.get_session_file().unwrap().to_owned();
    let before = fs::read(&file).unwrap();
    let mut empty = SessionManager::in_memory(Some("/work")).unwrap();
    let session = native.get_session_id().to_owned();
    for leaf in ["", "missing"] {
        let error = native.create_branched_session(leaf).unwrap_err();
        assert_eq!(error.to_string(), format!("Entry {leaf} not found"));
        assert_eq!(native.get_session_id(), session);
    }
    let session = empty.get_session_id().to_owned();
    let error = empty.create_branched_session("absent").unwrap_err();
    assert_eq!(error.to_string(), "Entry absent not found");
    assert_eq!(empty.get_session_id(), session);
    assert_eq!(ids(&native), [""]);
    assert_eq!(fs::read(&file).unwrap(), before);
    native
        .append_message(json!({"role": "assistant", "content": "later", "timestamp": 2}))
        .unwrap();
    assert_eq!(records(&file).len(), 3);
}

#[test]
fn extraction_keeps_unknown_records_and_unreached_fields() {
    let scratch = Scratch::new().unwrap();
    let records = [
        json!({"type": "weird", "id": "w", "parentId": "ghost", "extra": [1, 1, null], "n": null}),
        json!({"type": "message", "id": "m", "parentId": "w", "timestamp": "t",
            "message": {"role": "stranger", "extra": null, "content": [1, 1]}}),
        json!({"type": "custom", "id": "c", "parentId": "m", "timestamp": "t",
            "customType": "k", "data": {"x": null}}),
        json!({"type": "thinking_level_change", "id": "t", "parentId": "c", "timestamp": "t",
            "thinkingLevel": "high"}),
        json!({"type": "session_info", "id": "s", "parentId": "t", "timestamp": "t", "name": "n"}),
        json!({"type": "model_change", "id": "mc", "parentId": "s", "timestamp": "t",
            "provider": "p", "modelId": "m", "extra": null}),
        json!({"type": "compaction", "id": "cp", "parentId": "mc", "timestamp": "t",
            "summary": "s", "firstKeptEntryId": "m", "tokensBefore": 1, "details": {"k": [1, 1]}}),
        json!({"type": "branch_summary", "id": "bs", "parentId": "cp", "timestamp": "t",
            "fromId": "m", "summary": "s", "extra": [null]}),
        json!({"type": "custom_message", "id": "cm", "parentId": "bs", "timestamp": "t",
            "customType": "k", "content": "c", "display": true, "details": null}),
    ];
    let mut manager = open(&scratch, "raw.jsonl", &records);
    manager.create_branched_session("cm").unwrap();
    let kept: Vec<Value> = manager.get_entries().into_iter().cloned().collect();
    assert_eq!(kept, records);
    let order: Vec<&str> = kept[0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(order, ["type", "id", "parentId", "extra", "n"]);
}

#[test]
fn extracted_context_reuses_local_timestamp_conversion() {
    let name = "extracted_context_reuses_local_timestamp_conversion";
    let selection = maestro_test_process::selection();
    let zone = selection.is_child().then(|| {
        ["America/New_York", "UTC"]
            .into_iter()
            .find(|zone| selection.runs_case(zone))
            .expect("selected timezone")
    });
    let Some(zone) = zone else {
        if !maestro_test_process::local_zone_follows_tz() {
            return;
        }
        for zone in ["America/New_York", "UTC"] {
            let output = maestro_test_process::TestProcess::current(name)
                .unwrap()
                .nocapture()
                .env("TZ", zone)
                .case(zone)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{zone}\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let utc = zone == "UTC";
    assert_eq!(chrono::Local::now().offset().local_minus_utc() == 0, utc);
    let local_millis = if utc {
        1_735_693_200_000_f64
    } else {
        1_735_711_200_000_f64
    };
    let scratch = Scratch::new().unwrap();
    for (time, millis) in [
        ("2025-01-01T01:00:00", local_millis),
        ("2025-01-01T01:00:00+02:00", 1_735_686_000_000.0),
        ("2025-01-01", 1_735_689_600_000.0),
    ] {
        let compaction = json!({"type": "compaction", "id": "c", "parentId": "a", "timestamp": time,
            "summary": "s", "firstKeptEntryId": "a", "tokensBefore": 1});
        let mut manager = open(
            &scratch,
            "time.jsonl",
            &[message("a", None, "user"), compaction],
        );
        manager.create_branched_session("c").unwrap();
        assert_eq!(manager.get_entry("c").unwrap()["timestamp"], time);
        let context = manager.build_session_context().unwrap();
        assert_eq!(
            context.messages[0]["timestamp"].as_f64(),
            Some(millis),
            "{time}"
        );
    }
}

#[test]
fn extraction_retains_only_resolved_path_labels() {
    let mut manager = SessionManager::in_memory(Some("/work")).unwrap();
    let a = manager
        .append_message(json!({"role": "user", "content": "a", "timestamp": 1}))
        .unwrap();
    let b = manager
        .append_message(json!({"role": "user", "content": "b", "timestamp": 2}))
        .unwrap();
    manager
        .append_label_change(&a, Presence::Present("A"))
        .unwrap();
    manager
        .append_label_change(&b, Presence::Present("B"))
        .unwrap();
    manager.branch(&a).unwrap();
    let c = manager
        .append_message(json!({"role": "user", "content": "c", "timestamp": 3}))
        .unwrap();
    let stamp = manager.get_tree()[0].label_timestamp.cloned();
    manager.create_branched_session(&c).unwrap();
    assert_eq!(manager.get_label(&a), Some(&json!("A")));
    assert_eq!(manager.get_label(&b), None);
    assert_eq!(manager.get_tree()[0].label_timestamp.cloned(), stamp);
    assert_eq!(manager.get_tree()[0].label, Some(&json!("A")));
    assert_eq!(manager.get_entries().len(), 3);
}

/// Targets, values and timestamps of the recreated label entries, in order.
fn recreated(manager: &SessionManager) -> Vec<(String, Value, Value)> {
    manager
        .get_entries()
        .into_iter()
        .filter(|entry| entry["type"] == "label")
        .map(|e| {
            (
                e["targetId"].as_str().unwrap().to_owned(),
                e["label"].clone(),
                e["timestamp"].clone(),
            )
        })
        .collect()
}

#[test]
fn extraction_preserves_label_order_and_last_timestamp() {
    let scratch = Scratch::new().unwrap();
    let local = "2025-01-01T01:00:00";
    let base = [message("a", None, "user"), message("b", Some("a"), "user")];
    let replaced = [
        label("l1", Some("b"), "a", json!("A1"), "t1"),
        label("l2", Some("l1"), "b", json!("B1"), "t2"),
        label("l3", Some("l2"), "a", json!("A2"), local),
    ];
    let reinserted = [
        label("l1", Some("b"), "a", json!("A1"), "t1"),
        label("l2", Some("l1"), "b", json!("B1"), "t2"),
        label("l3", Some("l2"), "a", json!(""), "t3"),
        label("l4", Some("l3"), "a", json!("A2"), local),
    ];
    for (name, labels, expected) in [
        (
            "replaced",
            &replaced[..],
            [("a", "A2", local), ("b", "B1", "t2")],
        ),
        (
            "reinserted",
            &reinserted[..],
            [("b", "B1", "t2"), ("a", "A2", local)],
        ),
    ] {
        let mut records = base.to_vec();
        records.extend_from_slice(labels);
        let mut manager = open(&scratch, name, &records);
        let leaf = manager.get_leaf_id().unwrap().to_owned();
        manager.create_branched_session(&leaf).unwrap();
        let expected: Vec<_> = expected
            .iter()
            .map(|(target, text, time)| ((*target).to_owned(), json!(text), json!(time)))
            .collect();
        assert_eq!(recreated(&manager), expected, "{name}");
    }
}

#[test]
fn extraction_omits_cleared_labels() {
    let scratch = Scratch::new().unwrap();
    for clear in [json!(""), Value::Null, json!(false), json!(0)] {
        let records = [
            message("a", None, "user"),
            label("l1", Some("a"), "a", json!("A"), "t1"),
            label("l2", Some("l1"), "a", clear.clone(), "t2"),
        ];
        let mut manager = open(&scratch, "cleared", &records);
        manager.create_branched_session("l2").unwrap();
        assert_eq!(ids(&manager), ["a"], "{clear}");
        assert_eq!(manager.get_label("a"), None);
    }
}

#[test]
fn extraction_omits_a_missing_label_timestamp_but_keeps_null() {
    let scratch = Scratch::new().unwrap();
    let base = [message("a", None, "user")];
    let first = label("l1", Some("a"), "a", json!("A1"), "t1");
    let missing = json!({"type": "label", "id": "l2", "parentId": "l1", "targetId": "a",
        "label": "A2"});
    let mut null_stamp = label("l2", Some("l1"), "a", json!("A2"), "t2");
    null_stamp["timestamp"] = Value::Null;
    for (name, last, expected) in [
        ("missing", missing, None),
        ("null", null_stamp, Some(Value::Null)),
    ] {
        let mut records = base.to_vec();
        records.extend([first.clone(), last]);
        let mut manager = open(&scratch, name, &records);
        manager.create_branched_session("l2").unwrap();
        let kept = manager.get_entries();
        let recreated = kept.last().unwrap();
        assert_eq!(recreated["label"], "A2", "{name}");
        assert_eq!(recreated.get("timestamp"), expected.as_ref(), "{name}");
    }
}

#[test]
fn label_only_extraction_produces_an_empty_history() {
    let scratch = Scratch::new().unwrap();
    let only = label("l1", None, "ghost", json!("L"), "t1");
    let mut manager = open(&scratch, "only.jsonl", &[only]);
    let path = manager.create_branched_session("l1").unwrap().unwrap();
    assert!(manager.get_entries().is_empty());
    assert_eq!(manager.get_leaf_id(), None);
    assert!(!fs::exists(&path).unwrap());
    manager
        .append_message(json!({"role": "assistant", "content": "x", "timestamp": 1}))
        .unwrap();
    let written = records(&path);
    assert_eq!(written.len(), 2);
    assert_eq!(written[0]["type"], "session");
}

#[test]
fn extraction_recreates_labels_after_the_retained_leaf() {
    let scratch = Scratch::new().unwrap();
    let mut old = label("l1", Some("b"), "a", json!("A"), "t1");
    old["note"] = json!(1);
    let records = [
        message("a", None, "user"),
        message("b", Some("a"), "user"),
        old,
    ];
    let mut manager = open(&scratch, "leaf.jsonl", &records);
    manager.create_branched_session("l1").unwrap();
    let kept: Vec<Value> = manager.get_entries().into_iter().cloned().collect();
    assert_eq!(kept.len(), 3);
    let new = &kept[2];
    assert_ne!(new["id"], "l1");
    assert_eq!(new["parentId"], "b");
    let order: Vec<&str> = new
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        order,
        ["type", "id", "parentId", "timestamp", "targetId", "label"]
    );
    assert_eq!(manager.get_leaf_id(), new["id"].as_str());
}

#[test]
fn extraction_reconnects_removed_labels_without_mutating_old_copies() {
    let scratch = Scratch::new().unwrap();
    let cases: [Vec<Value>; 3] = [
        vec![
            message("u1", None, "user"),
            label("l1", Some("u1"), "u1", json!("x"), "t"),
            message("u2", Some("l1"), "assistant"),
        ],
        vec![
            message("u1", None, "user"),
            label("l1", Some("u1"), "u1", json!("x"), "t"),
            label("l2", Some("l1"), "u1", json!("y"), "t"),
            message("u2", Some("l2"), "assistant"),
        ],
        vec![
            label("l1", None, "ghost", json!("x"), "t"),
            message("u1", Some("l1"), "user"),
            message("u2", Some("u1"), "assistant"),
        ],
    ];
    for (index, history) in cases.iter().enumerate() {
        let name = format!("gap{index}.jsonl");
        let source = write(&scratch, &name, history);
        let before = fs::read(&source).unwrap();
        let mut manager =
            SessionManager::open(&source, Some(scratch.path().to_str().unwrap()), None).unwrap();
        let target = if index == 2 { "u1" } else { "u2" };
        let old = manager.get_entry(target).cloned().unwrap();
        let path = manager.create_branched_session("u2").unwrap().unwrap();
        let context = manager.build_session_context().unwrap();
        let texts: Vec<&Value> = context.messages.iter().map(|m| &m["content"]).collect();
        assert_eq!(texts, [&json!("u1"), &json!("u2")], "case {index}");
        let wire = records(&path);
        assert_eq!(wire.len(), manager.get_entries().len() + 1, "case {index}");
        assert_eq!(wire[1]["parentId"], Value::Null, "case {index}");
        assert_ne!(
            old["parentId"],
            manager.get_entry(target).unwrap()["parentId"]
        );
        assert_eq!(fs::read(&source).unwrap(), before);
    }
}

/// Header, entries, identity, leaf, label and label timestamp of a history.
type Observed = (
    Option<Value>,
    Vec<Value>,
    String,
    Option<String>,
    Option<Value>,
    Option<Value>,
);

/// Owned snapshot of the observable history state with the label of `target`.
fn observe(manager: &SessionManager, target: &str) -> Observed {
    (
        manager.get_header().cloned(),
        manager.get_entries().into_iter().cloned().collect(),
        manager.get_session_id().to_owned(),
        manager.get_leaf_id().map(str::to_owned),
        manager.get_label(target).cloned(),
        manager.get_tree()[0].label_timestamp.cloned(),
    )
}

#[test]
fn populated_memory_rejects_extraction_without_losing_history() {
    let scratch = Scratch::new().unwrap();
    let mut manager = SessionManager::in_memory(Some("/work")).unwrap();
    let selected = below(scratch.path(), "absent.jsonl");
    manager.set_session_file(&selected).unwrap();
    let user = manager
        .append_message(json!({"role": "user", "content": "question", "timestamp": 1}))
        .unwrap();
    manager
        .append_message(json!({"role": "assistant", "content": "answer", "timestamp": 2}))
        .unwrap();
    manager
        .append_label_change(&user, Presence::Present("mark"))
        .unwrap();
    let before = observe(&manager, &user);
    assert!(before.5.is_some());
    for leaf in ["missing", ""] {
        let error = manager.create_branched_session(leaf).unwrap_err();
        assert_eq!(error.to_string(), format!("Entry {leaf} not found"));
        assert_eq!(observe(&manager, &user), before);
        assert_eq!(manager.get_session_file(), Some(selected.as_str()));
    }
    let entries = before.1;
    let prior_leaf = manager.get_leaf_id().unwrap().to_owned();
    let later = manager
        .append_message(json!({"role": "user", "content": "later", "timestamp": 3}))
        .unwrap();
    assert_eq!(manager.get_entry(&later).unwrap()["parentId"], prior_leaf);
    assert_eq!(manager.get_leaf_id(), Some(later.as_str()));
    let all: Vec<Value> = manager.get_entries().into_iter().cloned().collect();
    assert_eq!(all[..entries.len()], entries);
    assert_eq!(all.len(), entries.len() + 1);
    assert!(!fs::exists(&selected).unwrap());
}

#[test]
fn user_only_extraction_publishes_once_when_assistant_arrives() {
    let scratch = Scratch::new().unwrap();
    let mut manager =
        SessionManager::create("/work", Some(scratch.path().to_str().unwrap()), "unused").unwrap();
    let mut first = None;
    for (index, role) in ["user", "assistant", "user", "assistant"]
        .into_iter()
        .enumerate()
    {
        let id = manager
            .append_message(json!({"role": role, "content": role, "timestamp": index}))
            .unwrap();
        first.get_or_insert(id);
    }
    let user = first.unwrap();
    let source = manager.get_session_file().unwrap().to_owned();
    let before = fs::read(&source).unwrap();
    assert_eq!(records(&source).len(), 5);
    let path = manager.create_branched_session(&user).unwrap().unwrap();
    assert_eq!(manager.get_session_file(), Some(path.as_str()));
    assert_ne!(path, source);
    assert!(!fs::exists(&path).unwrap());
    manager.append_custom_entry("k", Presence::Missing).unwrap();
    assert!(!fs::exists(&path).unwrap());
    manager
        .append_message(json!({"role": "assistant", "content": "a", "timestamp": 2}))
        .unwrap();
    let published = records(&path);
    assert_eq!(published.len(), 4);
    let types: Vec<&str> = published
        .iter()
        .map(|r| r["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["session", "message", "custom", "message"]);
    manager
        .append_message(json!({"role": "user", "content": "v", "timestamp": 3}))
        .unwrap();
    let written = records(&path);
    assert_eq!(written.len(), 5);
    assert_eq!(written.iter().filter(|r| r["type"] == "session").count(), 1);
    let wire: Vec<&str> = written[1..]
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(wire, ids(&manager));
    let mut unique = wire.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), wire.len());
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn assistant_extraction_publishes_before_returning() {
    let scratch = Scratch::new().unwrap();
    let mut manager =
        SessionManager::create("/work", Some(scratch.path().to_str().unwrap()), "unused").unwrap();
    let previous = manager.get_session_file().unwrap().to_owned();
    manager
        .append_message(json!({"role": "user", "content": "u", "timestamp": 1}))
        .unwrap();
    let assistant = manager
        .append_message(json!({"role": "assistant", "content": "a", "timestamp": 2}))
        .unwrap();
    let path = manager
        .create_branched_session(&assistant)
        .unwrap()
        .unwrap();
    assert_ne!(path, previous);
    let written = records(&path);
    assert_eq!(written.len(), 3);
    assert_eq!(written[0]["parentSession"], previous);
    assert_eq!(written[0]["cwd"], "/work");
    assert_eq!(written[0]["id"], manager.get_session_id());
    assert_v7(manager.get_session_id());
    let contents: Vec<&Value> = written[1..]
        .iter()
        .map(|r| &r["message"]["content"])
        .collect();
    assert_eq!(contents, [&json!("u"), &json!("a")]);
    manager
        .append_message(json!({"role": "user", "content": "v", "timestamp": 3}))
        .unwrap();
    assert_eq!(records(&path).len(), 4);
}

#[test]
fn extraction_uses_resolved_labels_after_failed_label_publication() {
    let scratch = Scratch::new().unwrap();
    let (mut manager, first) = broken_history(&scratch);
    let leaf = manager.get_leaf_id().unwrap().to_owned();
    let file = manager.get_session_file().unwrap().to_owned();
    fs::remove_dir(&file).unwrap();
    manager
        .append_label_change(&first, Presence::Present("old"))
        .unwrap();
    let stamp = manager.get_tree()[0].label_timestamp.cloned().unwrap();
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    manager
        .append_label_change(&first, Presence::Present("new"))
        .unwrap_err();
    assert_eq!(manager.get_entries().last().unwrap()["label"], "new");
    let path = manager.create_branched_session(&leaf).unwrap().unwrap();
    let written = records(&path);
    let recreated = written.last().unwrap();
    assert_eq!(recreated["type"], "label");
    assert_eq!(recreated["label"], "old");
    assert_eq!(recreated["timestamp"], stamp);
}

#[test]
fn extraction_defers_role_reads_to_native_publication() {
    let scratch = Scratch::new().unwrap();
    let null_message = json!({"type": "message", "id": "n", "parentId": null, "timestamp": "t",
        "message": null});
    let mut before = open(
        &scratch,
        "before.jsonl",
        std::slice::from_ref(&null_message),
    );
    let session = before.get_session_id().to_owned();
    let error = before.create_branched_session("n").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_ne!(before.get_session_id(), session);
    assert_eq!(ids(&before), ["n"]);
    let mut after_null = null_message;
    after_null["id"] = json!("z");
    after_null["parentId"] = json!("a");
    let records_after = [message("a", None, "assistant"), after_null.clone()];
    let mut after = open(&scratch, "after.jsonl", &records_after);
    let path = after.create_branched_session("z").unwrap().unwrap();
    assert_eq!(records(&path).len(), 3);
    let mut memory = SessionManager::in_memory(Some("/work")).unwrap();
    let id = memory.append_message(Value::Null).unwrap();
    assert_eq!(memory.create_branched_session(&id).unwrap(), None);
    memory.build_session_context().unwrap_err();
}

/// Fork into the scratch directory.
fn fork(scratch: &Scratch, source: &str) -> SessionManager {
    let target = below(scratch.path(), "target");
    SessionManager::fork_from(source, "/target-cwd", Some(&target), "unused").unwrap()
}

/// Message record without identity.
fn plain(role: &str, text: &str) -> Value {
    json!({"type": "message", "timestamp": "2025-01-01T00:00:00.000Z",
        "message": {"role": role, "content": text, "timestamp": 1}})
}

#[test]
fn legacy_fork_converts_before_stamping_the_current_header() {
    let scratch = Scratch::new().unwrap();
    let lines = [plain("user", "first"), plain("assistant", "last")];
    let headers = [
        json!({"type": "session", "id": "s", "version": 1, "cwd": "/src", "timestamp": "t"}),
        json!({"type": "session", "id": "s", "cwd": "/src", "timestamp": "t"}),
        json!({"type": "session", "id": "s", "version": null, "cwd": "/src", "timestamp": "t"}),
    ];
    for (index, header) in headers.into_iter().enumerate() {
        let source = legacy(&scratch, &format!("v{index}.jsonl"), &header, &lines);
        let before = fs::read(&source).unwrap();
        let manager = fork(&scratch, &source);
        let context = manager.build_session_context().unwrap();
        let texts: Vec<&Value> = context.messages.iter().map(|m| &m["content"]).collect();
        assert_eq!(texts, [&json!("first"), &json!("last")], "case {index}");
        assert_eq!(manager.get_header().unwrap()["version"], 3);
        let entries = manager.get_entries();
        assert_eq!(entries[1]["parentId"], entries[0]["id"]);
        assert_eq!(fs::read(&source).unwrap(), before);
    }
    let hook = json!({"type": "session", "id": "s", "version": 2, "cwd": "/src", "timestamp": "t"});
    let mut hooked = message("h", None, "hookMessage");
    hooked["message"]["customType"] = json!("k");
    let source = legacy(&scratch, "v2.jsonl", &hook, &[hooked]);
    let manager = fork(&scratch, &source);
    assert_eq!(manager.get_entry("h").unwrap()["message"]["role"], "custom");
    let current =
        json!({"type": "session", "id": "s", "version": 3, "cwd": "/src", "timestamp": "t"});
    let source = legacy(
        &scratch,
        "v3.jsonl",
        &current,
        &[message("h", None, "hookMessage")],
    );
    let manager = fork(&scratch, &source);
    assert_eq!(
        manager.get_entry("h").unwrap()["message"]["role"],
        "hookMessage"
    );
}

#[test]
fn fork_preserves_current_and_future_nonheader_records() {
    let scratch = Scratch::new().unwrap();
    let second =
        json!({"type": "session", "version": 1, "id": "again", "cwd": "/x", "timestamp": "t"});
    let body = [
        message("a", None, "hookMessage"),
        second,
        json!({"type": "unknown", "id": "u", "parentId": "a", "payload": {"k": [null]}}),
        label("l", Some("u"), "a", json!("L"), "t"),
    ];
    for version in [3, 99] {
        let header = json!({"type": "session", "id": "s", "version": version, "cwd": "/src", "timestamp": "t"});
        let source = legacy(&scratch, &format!("f{version}.jsonl"), &header, &body);
        let before = fs::read(&source).unwrap();
        let manager = fork(&scratch, &source);
        let kept: Vec<Value> = manager.get_entries().into_iter().cloned().collect();
        let expected: Vec<Value> = body
            .iter()
            .filter(|r| r["type"] != "session")
            .cloned()
            .collect();
        assert_eq!(kept, expected, "version {version}");
        assert_eq!(manager.get_header().unwrap()["version"], 3);
        let wire = records(manager.get_session_file().unwrap());
        assert_eq!(wire.len(), 4);
        assert_eq!(wire[1..], expected[..]);
        assert_eq!(fs::read(&source).unwrap(), before);
    }
}

#[test]
fn fork_writes_header_only_and_user_only_histories_immediately() {
    let scratch = Scratch::new().unwrap();
    let header =
        json!({"type": "session", "id": "s", "version": 3, "cwd": "/src", "timestamp": "t"});
    for (index, lines) in [vec![], vec![message("u", None, "user")]]
        .into_iter()
        .enumerate()
    {
        let source = legacy(&scratch, &format!("h{index}.jsonl"), &header, &lines);
        let mut manager = fork(&scratch, &source);
        let path = manager.get_session_file().unwrap().to_owned();
        assert_eq!(records(&path).len(), 1 + lines.len());
        manager
            .append_message(json!({"role": "user", "content": "next", "timestamp": 2}))
            .unwrap();
        assert_eq!(records(&path).len(), 2 + lines.len());
    }
}

#[test]
fn fork_uses_target_cwd_and_explicit_or_default_directory() {
    let scratch = Scratch::new().unwrap();
    let header =
        json!({"type": "session", "id": "s", "version": 3, "cwd": "/src", "timestamp": "t"});
    let source = legacy(&scratch, "s.jsonl", &header, &[message("u", None, "user")]);
    let agent = below(scratch.path(), "agent");
    let explicit = below(scratch.path(), "explicit");
    let manager =
        SessionManager::fork_from(&source, "/work/target", Some(&explicit), &agent).unwrap();
    assert_eq!(manager.get_session_dir(), explicit);
    assert!(!fs::exists(&agent).unwrap());
    let default = SessionManager::fork_from(&source, "/work/target", None, &agent).unwrap();
    let sessions = scratch.path().join("agent").join("sessions");
    let directory = below(&sessions, "--work-target--");
    assert_eq!(default.get_session_dir(), directory);
    for (manager, directory) in [
        (&manager, explicit.as_str()),
        (&default, directory.as_str()),
    ] {
        let path = manager.get_session_file().unwrap();
        assert!(path.starts_with(directory));
        let header = &records(path)[0];
        assert_eq!(header["cwd"], "/work/target");
        assert_eq!(header["parentSession"], source);
        assert_eq!(header["version"], 3);
        assert_v7(header["id"].as_str().unwrap());
        assert_eq!(manager.get_session_id(), header["id"]);
        assert_eq!(manager.get_cwd(), "/work/target");
    }
}

#[test]
fn fork_preserves_authored_source_parent_spelling() {
    let scratch = uncanonicalized_scratch();
    fs::create_dir(below(scratch.path(), "sub")).unwrap();
    let header =
        json!({"type": "session", "id": "s", "version": 3, "cwd": "/src", "timestamp": "t"});
    legacy(&scratch, "s.jsonl", &header, &[]);
    let source = below_keeping_dots(scratch.path(), "sub/../s.jsonl");
    let manager = fork(&scratch, &source);
    assert_eq!(manager.get_header().unwrap()["parentSession"], source);
}

#[test]
fn fork_reuses_raw_loader_admission_and_exact_invalid_source_error() {
    let scratch = Scratch::new().unwrap();
    let target = below(scratch.path(), "never");
    let header = r#"{"type":"session","id":"s","version":3,"cwd":"/src","timestamp":"t"}"#;
    let user = message("u", None, "user").to_string();
    let invalid = [
        ("missing", None),
        ("empty", Some(String::new())),
        ("garbage", Some("not json".to_owned())),
        ("headerless", Some(format!("{user}\n"))),
    ];
    for (name, text) in invalid {
        let source = below(scratch.path(), &format!("{name}.jsonl"));
        if let Some(text) = text {
            fs::write(&source, text).unwrap();
        }
        let error = SessionManager::fork_from(&source, "/t", Some(&target), "unused")
            .err()
            .unwrap();
        assert_eq!(
            error.to_string(),
            format!("Cannot fork: source session file is empty or invalid: {source}"),
            "{name}"
        );
        assert!(!fs::exists(&target).unwrap(), "{name}");
    }
    let tolerant = [
        ("null-first", format!("null\n{header}\n{user}\n")),
        ("leading", format!("garbage\n{header}\n{user}\n")),
        ("tail", format!("{header}\n{user}\n{{\"type\":")),
        ("bom", format!("\u{feff}{header}\n")),
    ];
    for (name, text) in tolerant {
        let source = below(scratch.path(), &format!("{name}.jsonl"));
        fs::write(&source, &text).unwrap();
        let manager = SessionManager::fork_from(&source, "/t", Some(&target), "unused").unwrap();
        assert_eq!(
            manager.get_entries().len(),
            usize::from(name != "bom"),
            "{name}"
        );
    }
}

#[test]
fn fork_uses_existing_compact_json_number_spelling() {
    let scratch = Scratch::new().unwrap();
    let header = r#"{"type":"session","id":"s","version":3,"cwd":"/src","timestamp":"t"}"#;
    let record = concat!(
        r#"{"type":"custom","id":"n","parentId":null,"customType":"k","#,
        r#""data":[1.0,-0.0,1e-7,1e-6,1e20,1e21,0.125],"text":"a\nb \u00e9 \"q\"","dup":1,"dup":2,"#,
        r#""list":[{"x":1},{"x":1}]}"#
    );
    let source = below(scratch.path(), "numbers.jsonl");
    fs::write(&source, format!("{header}\n{record}\n")).unwrap();
    let manager = fork(&scratch, &source);
    let text = fs::read_to_string(manager.get_session_file().unwrap()).unwrap();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1],
        concat!(
            r#"{"type":"custom","id":"n","parentId":null,"customType":"k","#,
            r#""data":[1,0,1e-7,0.000001,100000000000000000000,1e+21,0.125],"#,
            "\"text\":\"a\\nb \u{e9} \\\"q\\\"\",\"dup\":2,\"list\":[{\"x\":1},{\"x\":1}]}\n"
        )
    );
}

#[test]
fn extraction_reuses_the_cycle_safe_ancestry_walk() {
    let scratch = Scratch::new().unwrap();
    let records = [
        message("a", None, "user"),
        message("b", Some("a"), "user"),
        message("c", Some("c"), "user"),
        message("d", Some("e"), "user"),
        message("e", Some("d"), "user"),
    ];
    for (leaf, expected) in [
        ("b", vec!["a", "b"]),
        ("c", vec!["c"]),
        ("d", vec!["e", "d"]),
    ] {
        let mut manager = open(&scratch, "cycle.jsonl", &records);
        manager.create_branched_session(leaf).unwrap();
        assert_eq!(ids(&manager), expected, "{leaf}");
        let id = manager.get_leaf_id().unwrap().to_owned();
        manager.branch(&id).unwrap();
        manager.append_custom_entry("k", Presence::Missing).unwrap();
    }
}

#[test]
fn fork_into_an_empty_session_directory_fails_without_writing() {
    let scratch = Scratch::new().unwrap();
    let source = write(&scratch, "empty-dir.jsonl", &[message("a", None, "user")]);
    let listing = || {
        let mut names: Vec<_> = fs::read_dir(".")
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let before = listing();
    let error = SessionManager::fork_from(&source, "/t", Some(""), "unused")
        .err()
        .unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(listing(), before);
}
