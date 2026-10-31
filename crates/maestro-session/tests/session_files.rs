//! Native history operations through their public boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_session::session_manager::load_entries_from_file;
use serde_json::json;

use crate::support;
use maestro_request::presence::Presence;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_session::session_manager::{find_most_recent_session, get_default_session_dir};
use maestro_session::{NewSessionOptions, SessionManager};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use support::native::{below, below_keeping_dots, uncanonicalized_scratch};
use support::session_files::cases;

/// Streamed loading drops empty values before header admission and indexing.
#[test]
fn loading_skips_empty_records_before_header_and_between_entries() {
    let directory = Directory::new();
    let entries = [
        header(&json!("cwd")),
        json!({"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"first"}}),
        json!({"type":"message","id":"b","parentId":"a","message":{"role":"assistant","content":"second"}}),
    ];
    for empty in ["null", "false", "0", "-0", "0.0", "\"\""] {
        for prefix in ["", empty] {
            let content = format!(
                "{prefix}\n{}{}\n{}",
                jsonl(&entries[..2]),
                empty,
                jsonl(&entries[2..])
            );
            let path = directory.file("empty-records.jsonl", &content);
            assert_eq!(load_entries_from_file(&path).unwrap(), entries);
            let opened = SessionManager::open(&path, None, None).unwrap();
            assert_eq!(
                opened.get_entries(),
                entries[1..].iter().collect::<Vec<_>>()
            );
            let mut switched = SessionManager::in_memory(Some("cwd")).unwrap();
            switched.set_session_file(&path).unwrap();
            assert_eq!(
                switched.get_entries(),
                entries[1..].iter().collect::<Vec<_>>()
            );
            let forked =
                SessionManager::fork_from(&path, "cwd", Some(directory.text()), "unused").unwrap();
            assert_eq!(
                forked.get_entries(),
                entries[1..].iter().collect::<Vec<_>>()
            );
            assert_eq!(
                maestro_session::parse_session_entries(&content).len(),
                if prefix.is_empty() { 4 } else { 5 }
            );
        }
    }
}

#[test]
fn file_loading_keeps_raw_lines_after_header_admission() {
    let directory = Directory::new();
    for case in cases("file_loading_keeps_raw_lines_after_header_admission") {
        let path = directory.path().join(&case.id);
        if let Some(bytes) = case.input.get("bytes") {
            let bytes: Vec<u8> = serde_json::from_value(bytes.clone()).unwrap();
            fs::write(&path, bytes).unwrap();
        } else if let Some(content) = case.input["content"].as_str() {
            fs::write(&path, content).unwrap();
        }
        assert_eq!(
            json!({"ok": load_entries_from_file(path.to_str().unwrap()).unwrap()}),
            case.expected,
            "{}",
            case.id
        );
    }
}

#[test]
fn new_histories_publish_only_at_first_assistant() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let dir = scratch.path().to_path_buf();
    let mut manager =
        maestro_session::SessionManager::create("authored", Some(dir.to_str().unwrap()), "unused")
            .unwrap();
    let path = manager.get_session_file().unwrap().to_owned();
    manager
        .append_message(json!({"role":"user","content":"first"}))
        .unwrap();
    assert!(!std::path::Path::new(&path).exists());
    manager
        .append_message(json!({"role":"assistant","content":"second"}))
        .unwrap();
    assert_eq!(load_entries_from_file(&path).unwrap().len(), 3);
    manager
        .append_message(json!({"role":"user","content":"third"}))
        .unwrap();
    let records = load_entries_from_file(&path).unwrap();
    assert_eq!(records.len(), 4);
    assert_eq!(records[1]["message"]["content"], "first");
    assert_eq!(records[2]["message"]["content"], "second");
    assert_eq!(records[3]["message"]["content"], "third");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn published_histories_never_republish_the_header() {
    for (content, users) in [
        ("", 0),
        (
            "{\"type\":\"session\",\"version\":3,\"id\":\"existing\",\"cwd\":\"stored\"}\n",
            0,
        ),
        (
            "{\"type\":\"session\",\"version\":3,\"id\":\"existing\",\"cwd\":\"stored\"}\n{\"type\":\"message\",\"id\":\"already\",\"message\":{\"role\":\"user\",\"content\":\"existing-user\"}}\n",
            1,
        ),
    ] {
        let file = maestro_test_directory::temporary_root()
            .join(format!("maestro-published-{}.jsonl", uuid::Uuid::new_v4()));
        std::fs::write(&file, content).unwrap();
        let mut manager =
            maestro_session::SessionManager::open(file.to_str().unwrap(), None, Some("override"))
                .unwrap();
        manager
            .append_message(json!({"role":"user","content":"first"}))
            .unwrap();
        manager
            .append_message(json!({"role":"assistant","content":"second"}))
            .unwrap();
        let reopened =
            maestro_session::SessionManager::open(file.to_str().unwrap(), None, None).unwrap();
        assert_eq!(reopened.get_entries().len(), users + 2);
        let records = load_entries_from_file(file.to_str().unwrap()).unwrap();
        assert_eq!(records.len(), users + 3);
        assert_eq!(records[users + 1]["message"]["content"], "first");
        assert_eq!(records[users + 2]["message"]["content"], "second");
        if users == 1 {
            assert_eq!(records[1]["message"]["content"], "existing-user");
        }
        std::fs::remove_file(file).unwrap();
    }
}

#[test]
fn continue_recent_loads_winner_or_creates_an_unpublished_history() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let dir = scratch.path().to_path_buf();
    let first = maestro_session::SessionManager::continue_recent(
        "caller",
        Some(dir.to_str().unwrap()),
        "unused",
    )
    .unwrap();
    assert!(!std::path::Path::new(first.get_session_file().unwrap()).exists());
    let path = dir.join("existing.jsonl");
    std::fs::write(
        &path,
        "{\"type\":\"session\",\"version\":3,\"id\":\"chosen\",\"cwd\":\"stored\"}\n",
    )
    .unwrap();
    let second = maestro_session::SessionManager::continue_recent(
        "caller",
        Some(dir.to_str().unwrap()),
        "unused",
    )
    .unwrap();
    assert_eq!(second.get_session_id(), "chosen");
    assert_eq!(second.get_cwd(), "caller");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn continuation_probe_is_raw_first_line_with_a_512_byte_bound() {
    for case in cases("continuation_probe_is_raw_first_line_with_a_512_byte_bound") {
        let directory = Directory::new();
        let path = directory.file("probe.jsonl", case.input["content"].as_str().unwrap());
        assert_eq!(
            json!({"ok": find_most_recent_session(directory.text()).as_deref() == Some(&path)}),
            case.expected,
            "{}",
            case.id
        );
    }
}

#[test]
fn default_directory_encodes_authored_cwd_before_joining() {
    if !maestro_test_process::selection().is_child() {
        let directory = Directory::new();
        let output = maestro_test_process::TestProcess::current(
            "default_directory_encodes_authored_cwd_before_joining",
        )
        .unwrap()
        .nocapture()
        .current_dir(directory.path())
        .case("*")
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        return;
    }
    for case in cases("default_directory_encodes_authored_cwd_before_joining") {
        let cwd = case.input["cwd"].as_str().unwrap();
        let agent = case.input["agentDir"].as_str().unwrap();
        let actual = get_default_session_dir(cwd, agent).unwrap();
        let safe = maestro_path::basename(&actual, None);
        let projected = match case.input["policy"].as_str().unwrap() {
            "posix" => maestro_path::posix::join(&[agent, "sessions", &safe]),
            "win32" => maestro_path::win32::join(&[agent, "sessions", &safe]),
            _ => panic!("unknown policy"),
        };
        assert_eq!(json!({"ok":projected}), case.expected, "{}", case.id);
    }
}

#[test]
fn new_session_replaces_native_path_without_publishing() {
    let directory = Directory::new();
    let mut manager = SessionManager::create("cwd", Some(directory.text()), "").unwrap();
    manager
        .append_message(json!({"role":"assistant","content":"old"}))
        .unwrap();
    let old = manager.get_session_file().unwrap().to_owned();
    let old_bytes = fs::read(&old).unwrap();
    let new = manager
        .new_session(NewSessionOptions {
            id: Some("chosen".into()),
            parent_session: Presence::Present(old.clone()),
        })
        .unwrap()
        .to_owned();
    assert!(!std::path::Path::new(&new).exists());
    assert_eq!(fs::read(&old).unwrap(), old_bytes);
    assert_eq!(manager.get_header().unwrap()["parentSession"], old);
    let timestamp = manager.get_header().unwrap()["timestamp"]
        .as_str()
        .unwrap()
        .replace([':', '.'], "-");
    assert_eq!(
        new,
        maestro_path::join(&[directory.text(), &format!("{timestamp}_chosen.jsonl")])
    );
    assert!(manager.get_entries().is_empty());
}

#[test]
fn loaded_records_defer_message_reads_until_the_consuming_operation() {
    let directory = Directory::new();
    for message in [
        None,
        Some(Value::Null),
        Some(json!({"role":"future","payload":[1,2,1]})),
    ] {
        let mut entry = json!({"type":"message","id":"a","parentId":null});
        if let Some(message) = &message {
            entry["message"] = message.clone();
        }
        let path = directory.file("late.jsonl", jsonl(&[header(&json!("cwd")), entry.clone()]));
        let manager = SessionManager::open(&path, None, None).unwrap();
        assert_eq!(manager.get_entry("a"), Some(&entry));
        if message.as_ref().is_none_or(Value::is_null) {
            assert!(manager.build_session_context().is_err());
        } else {
            assert_eq!(
                manager.build_session_context().unwrap().messages,
                vec![entry["message"].clone()]
            );
        }
    }
}

#[test]
fn switch_rebuilds_index_and_preserves_manager_cwd() {
    let directory = Directory::new();
    let mut manager = SessionManager::create("original", Some(directory.text()), "").unwrap();
    let old = manager.append_message(json!({"role":"user"})).unwrap();
    let entries = vec![
        header(&json!("stored")),
        json!({"type":"future","id":"same","value":"first"}),
        header(&json!("second")),
        json!({"type":"future","id":"same","value":"last"}),
        json!({"type":"future","id":"leaf","value":"end"}),
    ];
    let path = directory.file("switch.jsonl", jsonl(&entries));
    manager.set_session_file(&path).unwrap();
    assert_eq!(manager.get_cwd(), "original");
    assert!(manager.get_entry(&old).is_none());
    assert_eq!(manager.get_entry("same").unwrap()["value"], "last");
    assert_eq!(manager.get_leaf_id(), Some("leaf"));
    assert_eq!(
        manager
            .get_entries()
            .iter()
            .map(|entry| entry["value"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["first", "last", "end"]
    );
}

#[test]
fn opening_selects_override_then_header_then_runtime_cwd() {
    let directory = Directory::new();
    for stored in [
        Some(json!("stored")),
        Some(json!("")),
        Some(Value::Null),
        None,
    ] {
        let mut record = header(&Value::Null);
        if let Some(value) = &stored {
            record["cwd"] = value.clone();
        } else {
            record.as_object_mut().unwrap().shift_remove("cwd");
        }
        let path = directory.file("cwd.jsonl", jsonl(&[record]));
        for explicit in [Some("override"), Some(""), None] {
            let manager = SessionManager::open(&path, None, explicit).unwrap();
            let runtime = std::env::current_dir().unwrap();
            let expected = explicit
                .or_else(|| stored.as_ref().and_then(Value::as_str))
                .unwrap_or(runtime.to_str().unwrap());
            assert_eq!(manager.get_cwd(), expected);
            assert_eq!(manager.get_session_dir(), directory.text());
        }
        let manager = SessionManager::open(&path, Some(""), Some("override")).unwrap();
        assert_eq!(manager.get_session_dir(), "");
    }
}

#[test]
fn opening_legacy_history_assigns_ids_before_indexing() {
    let directory = Directory::new();
    let path = directory.file(
        "legacy.jsonl",
        jsonl(&[
            json!({"type":"session","id":"old","cwd":"cwd"}),
            json!({"type":"message","message":{"role":"hookMessage","content":"one"}}),
            json!({"type":"message","message":{"role":"user","content":"two"}}),
        ]),
    );
    let manager = SessionManager::open(&path, None, None).unwrap();
    assert_eq!(manager.get_header().unwrap()["version"], 3);
    assert_eq!(
        manager
            .build_session_context()
            .unwrap()
            .messages
            .iter()
            .map(|message| message["content"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    for entry in manager.get_entries() {
        assert_eq!(
            manager.get_entry(entry["id"].as_str().unwrap()),
            Some(entry)
        );
    }
    assert_eq!(
        load_entries_from_file(&path).unwrap()[1]["message"]["role"],
        "custom"
    );
}

#[test]
fn indexing_validates_only_consumed_entry_identity() {
    let directory = Directory::new();
    for entry in [
        json!({"type":"future"}),
        json!({"type":"future","id":null}),
        json!({"type":"future","id":7}),
        json!(7),
        json!([]),
    ] {
        let path = directory.file(
            "identity.jsonl",
            jsonl(&[header(&json!("cwd")), entry.clone()]),
        );
        assert_eq!(load_entries_from_file(&path).unwrap()[1], entry);
        assert_eq!(
            SessionManager::open(&path, None, None)
                .err()
                .unwrap()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
    for id in ["", "future"] {
        let entry = json!({"type":"future","id":id,"unknown":[1,2,1]});
        let path = directory.file(
            "identity.jsonl",
            jsonl(&[header(&json!("cwd")), entry.clone()]),
        );
        let manager = SessionManager::open(&path, None, None).unwrap();
        assert_eq!(manager.get_leaf_id(), Some(id));
        assert_eq!(manager.get_entry(id), Some(&entry));
    }
}

#[test]
fn cwd_override_bypasses_unneeded_stored_field_decoding() {
    let directory = Directory::new();
    let path = directory.file("cwd.jsonl", jsonl(&[header(&json!(7))]));
    assert_eq!(
        SessionManager::open(&path, None, None)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(
        SessionManager::open(&path, None, Some(""))
            .unwrap()
            .get_cwd(),
        ""
    );
}

#[test]
fn failed_switch_keeps_previous_history_consistent() {
    let directory = Directory::new();
    let mut manager = SessionManager::create("cwd", Some(directory.text()), "").unwrap();
    let leaf = manager
        .append_message(json!({"role":"assistant","content":"retained"}))
        .unwrap();
    let original = switch_snapshot(&manager, &leaf);
    let failed = [
        directory.text().to_owned(),
        directory.file(
            "version.jsonl",
            jsonl(&[json!({"type":"session","id":"bad","version":"wrong"})]),
        ),
        directory.file(
            "index.jsonl",
            jsonl(&[header(&json!("other")), json!({"type":"message","id":null})]),
        ),
    ];
    for selected in failed {
        assert!(manager.set_session_file(&selected).is_err());
        assert_eq!(switch_snapshot(&manager, &leaf), original);
    }
    let readonly = directory.file(
        "readonly.jsonl",
        jsonl(&[json!({"type":"session","version":2,"id":"readonly"})]),
    );
    let permissions = fs::metadata(&readonly).unwrap().permissions();
    let mut restricted = permissions.clone();
    restricted.set_readonly(true);
    fs::set_permissions(&readonly, restricted).unwrap();
    // A privileged Unix run writes the read-only file anyway, so there is nothing to check.
    #[cfg(unix)]
    if maestro_test_directory::deny(&readonly, 0o444)
        .unwrap()
        .is_none()
    {
        return;
    }
    let error = manager.set_session_file(&readonly).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    fs::set_permissions(&readonly, permissions).unwrap();
    assert_eq!(switch_snapshot(&manager, &leaf), original);
    let after_rewrite = directory.file(
        "rewritten.jsonl",
        jsonl(&[
            json!({"type":"session","version":2,"id":"index-error"}),
            json!({"type":"future","id":null}),
        ]),
    );
    assert!(manager.set_session_file(&after_rewrite).is_err());
    assert_eq!(
        load_entries_from_file(&after_rewrite).unwrap()[0]["version"],
        3
    );
    assert_eq!(switch_snapshot(&manager, &leaf), original);
}

#[test]
fn append_failure_keeps_memory_and_partial_bytes() {
    let directory = Directory::new();
    let mut manager = SessionManager::create("cwd", Some(directory.text()), "").unwrap();
    let path = manager.get_session_file().unwrap().to_owned();
    fs::create_dir(&path).unwrap();
    assert!(
        manager
            .append_message(json!({"role":"assistant","content":"accepted"}))
            .is_err()
    );
    assert_eq!(
        manager.get_leaf_entry().unwrap()["message"]["content"],
        "accepted"
    );
    assert_eq!(manager.get_entries().len(), 1);
    fs::remove_dir(&path).unwrap();
    manager
        .append_message(json!({"role":"user","content":"retry"}))
        .unwrap();
    assert_eq!(load_entries_from_file(&path).unwrap().len(), 3);
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(
        manager
            .append_message(json!({"role":"user","content":"failed-published"}))
            .is_err()
    );
    fs::remove_dir(&path).unwrap();
    manager
        .append_message(json!({"role":"user","content":"next"}))
        .unwrap();
    let bytes = fs::read_to_string(&path).unwrap();
    let records = maestro_session::parse_session_entries(&bytes);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["message"]["content"], "next");
    assert_eq!(manager.get_entries().len(), 4);
}

#[test]
fn published_append_failure_keeps_accepted_memory() {
    let directory = Directory::new();
    let path = directory.file("published.jsonl", jsonl(&[
        header(&json!("cwd")),
        json!({"type":"message","id":"published","parentId":null,"message":{"role":"assistant","content":"published"}}),
    ]));
    let mut manager = SessionManager::open(&path, None, None).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(
        manager
            .append_message(json!({"role":"user","content":"accepted-after-failure"}))
            .is_err()
    );
    assert_eq!(
        manager.get_leaf_entry().unwrap()["message"]["content"],
        "accepted-after-failure"
    );
    assert_eq!(manager.get_entries().len(), 2);
}

#[test]
fn continuation_uses_millisecond_ties_and_existing_catches() {
    let directory = Directory::new();
    assert!(find_most_recent_session(directory.text()).is_none());
    assert!(find_most_recent_session(directory.path().join("missing").to_str().unwrap()).is_none());
    directory.file("ignored.JSONL", jsonl(&[header(&json!("cwd"))]));
    directory.file("ignored.jsonl.lock", jsonl(&[header(&json!("cwd"))]));
    directory.file("invalid.jsonl", "bad");
    assert!(find_most_recent_session(directory.text()).is_none());
    let first = directory.file("first.jsonl", jsonl(&[header(&json!("a"))]));
    let second = directory.file("second.jsonl", jsonl(&[header(&json!("b"))]));
    let instant = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    set_modified(&first, instant + std::time::Duration::from_nanos(100));
    set_modified(&second, instant + std::time::Duration::from_micros(900));
    assert_eq!(find_most_recent_session(directory.text()).unwrap(), first);
    set_modified(&second, instant + std::time::Duration::from_millis(2));
    let winner = second.as_str();
    assert_eq!(
        find_most_recent_session(directory.text()).as_deref(),
        Some(winner)
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            directory.path().join("missing"),
            directory.path().join("dangling.jsonl"),
        )
        .unwrap();
        assert_eq!(
            find_most_recent_session(directory.text()).as_deref(),
            Some(winner)
        );
    }
    assert!(find_most_recent_session(&first).is_none());
    fs::create_dir(directory.path().join("unreadable.jsonl")).unwrap();
    assert_eq!(
        find_most_recent_session(directory.text()).as_deref(),
        Some(winner)
    );
    #[cfg(unix)]
    {
        removed_fifo_metadata_discards_selection(&directory);
    }
}

/// Equal modification times resume the first name in name order, whatever order
/// the file system lists.
#[test]
fn continuation_ties_resume_the_first_name_in_name_order() {
    let directory = Directory::new();
    let instant = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    // NTFS lists these case-insensitively (a, b, C, _x); name order is byte order.
    for name in ["b", "_x", "a", "C"] {
        let record = json!({"type": "session", "version": 3, "id": name, "cwd": "stored"});
        set_modified(
            directory.file(&format!("{name}.jsonl"), jsonl(&[record])),
            instant,
        );
    }
    let resumed = SessionManager::continue_recent("caller", Some(directory.text()), "unused");
    assert_eq!(resumed.unwrap().get_session_id(), "C");
    let mut order = Vec::new();
    while let Some(path) = find_most_recent_session(directory.text()) {
        fs::remove_file(&path).unwrap();
        order.push(PathBuf::from(path).file_stem().unwrap().to_owned());
    }
    assert_eq!(order, ["C", "_x", "a", "b"]);
}

#[test]
#[cfg(unix)]
fn continuation_probes_all_candidates_before_reading_metadata() {
    let directory = Directory::new();
    for name in ["first.jsonl", "second.jsonl"] {
        assert!(
            std::process::Command::new("mkfifo")
                .arg(directory.path().join(name))
                .status()
                .unwrap()
                .success()
        );
    }
    let paths = ["first.jsonl", "second.jsonl"].map(|name| directory.path().join(name));
    let producer = std::thread::spawn(move || {
        use std::io::Write;
        {
            let mut earlier = fs::OpenOptions::new().write(true).open(&paths[0]).unwrap();
            earlier
                .write_all(b"{\"type\":\"session\",\"id\":\"earlier\"}\n")
                .unwrap();
        }
        // Opening the later writer waits until the earlier probe has completed.
        let mut later = fs::OpenOptions::new().write(true).open(&paths[1]).unwrap();
        fs::remove_file(&paths[0]).unwrap();
        later
            .write_all(b"{\"type\":\"session\",\"id\":\"later\"}\n")
            .unwrap();
    });
    let selected = find_most_recent_session(directory.text());
    producer.join().unwrap();
    assert!(
        selected.is_none(),
        "metadata failure after all probes must discard selection: {selected:?}"
    );
}

/// Set a fixture's timestamp through a writable native handle.
fn set_modified(path: impl AsRef<std::path::Path>, time: std::time::SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

#[test]
fn default_constructor_routes_use_the_resolved_agent_root() {
    let directory = Directory::new();
    let created = SessionManager::create("/authored", None, directory.text()).unwrap();
    let continued = SessionManager::continue_recent("/different", None, directory.text()).unwrap();
    assert_eq!(
        created.get_session_dir(),
        maestro_path::join(&[directory.text(), "sessions", "--authored--"])
    );
    assert_eq!(
        continued.get_session_dir(),
        maestro_path::join(&[directory.text(), "sessions", "--different--"])
    );
    assert!(!std::path::Path::new(created.get_session_file().unwrap()).exists());
    assert!(!std::path::Path::new(continued.get_session_file().unwrap()).exists());
    for manager in [
        SessionManager::create("cwd", Some(directory.text()), "invalid"),
        SessionManager::continue_recent("cwd", Some(directory.text()), "invalid"),
    ] {
        assert_eq!(manager.unwrap().get_session_dir(), directory.text());
    }
}

#[test]
fn opening_resolves_once_before_loading_the_selected_file() {
    let directory = Directory::new();
    let path = directory.file("existing.jsonl", jsonl(&[header(&json!("stored"))]));
    let bytes = fs::read(&path).unwrap();
    let authored = directory.path().join("missing/../existing.jsonl");
    let manager = SessionManager::open(authored.to_str().unwrap(), None, None).unwrap();
    assert_eq!(manager.get_session_id(), "stored");
    assert_eq!(manager.get_cwd(), "stored");
    assert_eq!(manager.get_session_file(), Some(path.as_str()));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn session_paths_keep_authored_spelling_until_explicit_resolution() {
    let directory = Directory(uncanonicalized_scratch());
    let authored = below_keeping_dots(directory.path(), "./nested/..");
    let manager =
        SessionManager::create("relative/../authored", Some(&authored), "unused").unwrap();
    assert_eq!(manager.get_session_dir(), authored);
    assert_eq!(manager.get_cwd(), "relative/../authored");
    assert_eq!(
        maestro_path::dirname(manager.get_session_file().unwrap()),
        directory.text()
    );
    #[cfg(unix)]
    {
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(directory.path(), &alias).unwrap();
        let path = directory.file("linked.jsonl", jsonl(&[header(&json!("cwd"))]));
        let selected = alias.join("linked.jsonl");
        let manager = SessionManager::open(selected.to_str().unwrap(), None, None).unwrap();
        assert_eq!(manager.get_session_file(), Some(selected.to_str().unwrap()));
        assert_ne!(manager.get_session_file(), Some(path.as_str()));
    }
}

#[test]
fn default_directory_creates_only_the_encoded_location() {
    let directory = Directory::new();
    let path = get_default_session_dir("//authored:folder/界", directory.text()).unwrap();
    assert_eq!(
        path,
        maestro_path::join(&[directory.text(), "sessions", "---authored-folder-界--"])
    );
    assert!(std::path::Path::new(&path).is_dir());
    assert_eq!(
        get_default_session_dir("//authored:folder/界", directory.text()).unwrap(),
        path
    );
    assert_eq!(fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn directory_creation_errors_propagate_without_publishing() {
    let directory = Directory::new();
    let file = directory.file("parent", "unchanged");
    let child = below(directory.path(), "parent/child");
    let original = fs::create_dir_all(&child).unwrap_err();
    let actual = SessionManager::create("cwd", Some(&child), "unused")
        .err()
        .unwrap();
    assert_eq!(actual.raw_os_error(), original.raw_os_error());
    assert_eq!(fs::read_to_string(file).unwrap(), "unchanged");
}

#[test]
fn opening_missing_file_preserves_explicit_path_until_publication() {
    let directory = Directory::new();
    let path = directory.path().join("missing.jsonl");
    let mut manager = SessionManager::open(path.to_str().unwrap(), None, Some("cwd")).unwrap();
    assert_eq!(manager.get_session_file(), Some(path.to_str().unwrap()));
    manager.append_message(json!({"role":"user"})).unwrap();
    assert!(!path.exists());
    manager.append_message(json!({"role":"assistant"})).unwrap();
    assert_eq!(
        load_entries_from_file(path.to_str().unwrap())
            .unwrap()
            .len(),
        3
    );
    assert_eq!(fs::read_dir(directory.text()).unwrap().count(), 1);
    assert!(!path.with_extension("jsonl.lock").exists());
}

#[test]
fn opening_a_nonempty_invalid_file_refuses_it_and_leaves_its_bytes() {
    let directory = Directory::new();
    let invalid = [
        ("text.jsonl", "not a session\n".to_owned()),
        ("blank.jsonl", "\n".to_owned()),
        (
            "message.jsonl",
            jsonl(&[json!({"type":"message","id":"a","message":{"role":"user"}})]),
        ),
        (
            "nameless.jsonl",
            jsonl(&[json!({"type":"session","version":3})]),
        ),
    ];
    let mut memory = SessionManager::in_memory(Some("cwd")).unwrap();
    for (name, content) in invalid {
        let path = directory.file(name, &content);
        let error = SessionManager::open(&path, None, Some("cwd"))
            .err()
            .unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{name}");
        assert_eq!(
            error.to_string(),
            format!("Session file is not a valid session: {path}")
        );
        assert!(memory.set_session_file(&path).is_err(), "{name}");
        assert_eq!(fs::read_to_string(&path).unwrap(), content, "{name}");
    }
}

#[test]
fn opening_an_empty_file_initialises_it() {
    let directory = Directory::new();
    let path = directory.file("empty.jsonl", "");
    let manager = SessionManager::open(&path, None, Some("cwd")).unwrap();
    let stored = load_entries_from_file(&path).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["id"], manager.get_session_id());
}

#[test]
fn explicit_file_selection_in_memory_reads_without_writing() {
    let directory = Directory::new();
    let path = directory.file(
        "memory.jsonl",
        jsonl(&[
            json!({"type":"session","id":"legacy","version":1}),
            json!({"type":"message","message":{"role":"hookMessage","content":"changed"}}),
        ]),
    );
    let bytes = fs::read(&path).unwrap();
    let mut manager = SessionManager::in_memory(Some("cwd")).unwrap();
    manager.set_session_file(&path).unwrap();
    assert_eq!(manager.get_entries()[0]["message"]["role"], "custom");
    manager.append_message(json!({"role":"assistant"})).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!manager.is_persisted());
    assert_eq!(manager.get_session_file(), Some(path.as_str()));
    manager.new_session(NewSessionOptions::default());
    manager.append_message(json!({"role":"user"})).unwrap();
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn jsonl_uses_compact_finite_number_spelling() {
    for case in cases("jsonl_uses_compact_finite_number_spelling") {
        let directory = Directory::new();
        let mut manager = SessionManager::create("cwd", Some(directory.text()), "unused").unwrap();
        let numbers = [1.0, -0.0, 1e-7, 1e-6, 1e20, 1e21, 0.125];
        support::value::assert_value(
            &serde_json::to_value(numbers).unwrap(),
            &case.input["values"],
            &case.id,
        );
        manager
            .append_message(json!({"role":"assistant", "values":numbers}))
            .unwrap();
        let bytes = fs::read_to_string(manager.get_session_file().unwrap()).unwrap();
        let expected = case.expected["ok"]
            .as_str()
            .unwrap()
            .strip_prefix("{\"values\":")
            .unwrap();
        let actual = bytes.rsplit_once("\"values\":").unwrap().1;
        assert_eq!(
            actual.strip_suffix("}\n").unwrap(),
            expected.strip_suffix('\n').unwrap()
        );
        assert_eq!(bytes.lines().count(), 2);
    }
}

#[cfg(unix)]
/// Run one named test in a disposable removed working directory child.
fn removed_cwd_child(name: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return true;
    }
    let directory = Directory::new();
    let output = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .nocapture()
        .case("*")
        .env("MAESTRO_REMOVED_CWD_PARENT", directory.text())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    false
}

#[cfg(unix)]
/// Enter then unlink a working directory without altering the parent test process.
/// Returns a fixture directory and the owner of the removed one.
fn remove_cwd() -> (Directory, maestro_test_directory::Scratch) {
    let removed = maestro_test_directory::Scratch::new().unwrap();
    removed.enter_removed("removed").unwrap();
    (Directory::new(), removed)
}

#[cfg(unix)]
#[test]
fn absolute_open_avoids_unneeded_cwd_reads() {
    if !removed_cwd_child("absolute_open_avoids_unneeded_cwd_reads") {
        return;
    }
    let (directory, _removed) = remove_cwd();
    let path = directory.file("absolute.jsonl", jsonl(&[header(&json!("stored"))]));
    assert!(std::env::current_dir().is_err());
    assert_eq!(
        SessionManager::open(&path, None, Some(""))
            .unwrap()
            .get_cwd(),
        ""
    );
    assert_eq!(
        SessionManager::open(&path, None, None).unwrap().get_cwd(),
        "stored"
    );
}

#[cfg(unix)]
#[test]
fn opening_failure_precedence_preserves_native_read_errors() {
    if !removed_cwd_child("opening_failure_precedence_preserves_native_read_errors") {
        return;
    }
    let (directory, _removed) = remove_cwd();
    let path = directory.file("fallback.jsonl", jsonl(&[header(&Value::Null)]));
    let original = std::env::current_dir().unwrap_err();
    assert_eq!(
        SessionManager::open(&path, None, None)
            .err()
            .unwrap()
            .raw_os_error(),
        original.raw_os_error()
    );
    assert_eq!(
        SessionManager::open("relative.jsonl", None, Some("override"))
            .err()
            .unwrap()
            .raw_os_error(),
        original.raw_os_error()
    );
    let read = fs::read(directory.text()).unwrap_err();
    assert_eq!(
        SessionManager::open(directory.text(), None, None)
            .err()
            .unwrap()
            .raw_os_error(),
        read.raw_os_error()
    );
}

/// A uniquely named native directory removed after observations.
struct Directory(maestro_test_directory::Scratch);
impl Directory {
    /// Create a fresh directory without environment mutation.
    fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }
    /// The directory this test owns.
    fn path(&self) -> &std::path::Path {
        self.0.path()
    }
    /// Borrow its native spelling.
    fn text(&self) -> &str {
        self.0.path().to_str().unwrap()
    }
    /// Write fixture bytes to one selected file.
    fn file(&self, name: &str, bytes: impl AsRef<[u8]>) -> String {
        let path = self.0.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }
}
/// A current header whose cwd can be varied without incidental payload fields.
fn header(cwd: &Value) -> Value {
    json!({"type":"session", "version":3,"id":"stored","cwd":cwd})
}
/// Encode a sequence independently with ordinary serde for test setup.
fn jsonl(entries: &[Value]) -> String {
    entries
        .iter()
        .map(|entry| serde_json::to_string(entry).unwrap() + "\n")
        .collect()
}

/// Unlink the probed FIFO before the producer permits the reader to finish.
#[cfg(unix)]
fn removed_fifo_metadata_discards_selection(directory: &Directory) {
    let fifo = directory.path().join("stat-error.jsonl");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let producer = std::thread::spawn(move || {
        use std::io::Write;
        let mut file = fs::OpenOptions::new().write(true).open(&fifo).unwrap();
        fs::remove_file(&fifo).unwrap();
        file.write_all(b"{\"type\":\"session\",\"id\":\"fifo\"}\n")
            .unwrap();
    });
    let selected = find_most_recent_session(directory.text());
    producer.join().unwrap();
    assert!(selected.is_none());
}

/// Observe the current manager and lookup through the identity retained before a failed switch.
fn switch_snapshot(manager: &SessionManager, retained_id: &str) -> Value {
    json!({"file":manager.get_session_file(), "id":manager.get_session_id(),
        "header":manager.get_header(), "entries":manager.get_entries(),
        "leaf":manager.get_leaf_id(), "lookup":manager.get_entry(retained_id),
        "cwd":manager.get_cwd()})
}
