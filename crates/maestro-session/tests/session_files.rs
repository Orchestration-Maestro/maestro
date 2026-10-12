//! Native history operations through their public boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_session::session_manager::load_entries_from_file;
use serde_json::json;

/// Strict fixtures and disposable native directories.
mod support {
    pub mod session_files;
    pub mod value;
}
use maestro_request::presence::Presence;
use maestro_session::session_manager::{find_most_recent_session, get_default_session_dir};
use maestro_session::{NewSessionOptions, SessionManager};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use support::session_files::cases;

#[test]
fn file_loading_keeps_raw_lines_after_header_admission() {
    let directory = Directory::new();
    for case in cases("file_loading_keeps_raw_lines_after_header_admission") {
        let path = directory.path.join(&case.id);
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
    let dir = std::env::temp_dir().join(format!("maestro-history-{}", uuid::Uuid::new_v4()));
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
        ("garbage", 0),
        (
            "{\"type\":\"session\",\"version\":3,\"id\":\"existing\",\"cwd\":\"stored\"}\n",
            0,
        ),
        (
            "{\"type\":\"session\",\"version\":3,\"id\":\"existing\",\"cwd\":\"stored\"}\n{\"type\":\"message\",\"id\":\"already\",\"message\":{\"role\":\"user\",\"content\":\"existing-user\"}}\n",
            1,
        ),
    ] {
        let file =
            std::env::temp_dir().join(format!("maestro-published-{}.jsonl", uuid::Uuid::new_v4()));
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
    let dir = std::env::temp_dir().join(format!("maestro-continue-{}", uuid::Uuid::new_v4()));
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
    if std::env::var_os("MAESTRO_DIRECTORY_CHILD").is_none() {
        let directory = Directory::new();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "default_directory_encodes_authored_cwd_before_joining",
                "--nocapture",
            ])
            .current_dir(&directory.path)
            .env("MAESTRO_DIRECTORY_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
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
            record.as_object_mut().unwrap().remove("cwd");
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
fn continuation_uses_mtime_stable_ties_and_existing_catches() {
    let directory = Directory::new();
    assert!(find_most_recent_session(directory.text()).is_none());
    assert!(find_most_recent_session(directory.path.join("missing").to_str().unwrap()).is_none());
    directory.file("ignored.JSONL", jsonl(&[header(&json!("cwd"))]));
    directory.file("invalid.jsonl", "bad");
    assert!(find_most_recent_session(directory.text()).is_none());
    let first = directory.file("first.jsonl", jsonl(&[header(&json!("a"))]));
    directory.file("second.jsonl", jsonl(&[header(&json!("b"))]));
    let instant = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    let order: Vec<_> = fs::read_dir(directory.text())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name().unwrap() == "first.jsonl"
                || path.file_name().unwrap() == "second.jsonl"
        })
        .collect();
    for (position, path) in order.iter().enumerate() {
        let nanos = if position == 0 { 100 } else { 900_000 };
        fs::File::open(path)
            .unwrap()
            .set_modified(instant + std::time::Duration::from_nanos(nanos))
            .unwrap();
    }
    let winner = order[1].to_str().unwrap();
    assert_eq!(
        find_most_recent_session(directory.text()).unwrap(),
        order[0].to_str().unwrap()
    );
    fs::File::open(winner)
        .unwrap()
        .set_modified(instant + std::time::Duration::from_millis(2))
        .unwrap();
    assert_eq!(
        find_most_recent_session(directory.text()).as_deref(),
        Some(winner)
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            directory.path.join("missing"),
            directory.path.join("dangling.jsonl"),
        )
        .unwrap();
        assert_eq!(
            find_most_recent_session(directory.text()).as_deref(),
            Some(winner)
        );
    }
    assert!(find_most_recent_session(&first).is_none());
    fs::create_dir(directory.path.join("unreadable.jsonl")).unwrap();
    assert_eq!(
        find_most_recent_session(directory.text()).as_deref(),
        Some(winner)
    );
    #[cfg(unix)]
    {
        removed_fifo_metadata_discards_selection(&directory);
    }
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
    let authored = directory.path.join("missing/../existing.jsonl");
    let manager = SessionManager::open(authored.to_str().unwrap(), None, None).unwrap();
    assert_eq!(manager.get_session_id(), "stored");
    assert_eq!(manager.get_cwd(), "stored");
    assert_eq!(manager.get_session_file(), Some(path.as_str()));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn session_paths_keep_authored_spelling_until_explicit_resolution() {
    let directory = Directory::new();
    let authored = format!("{}/./nested/../", directory.text());
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
        let alias = directory.path.join("alias");
        std::os::unix::fs::symlink(&directory.path, &alias).unwrap();
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
    let child = format!("{file}/child");
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
    let path = directory.path.join("missing.jsonl");
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

/// Run one named test in a disposable removed working directory child.
fn removed_cwd_child(name: &str) -> bool {
    if std::env::var_os("MAESTRO_REMOVED_CWD_CHILD").is_some() {
        return true;
    }
    let directory = Directory::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("MAESTRO_REMOVED_CWD_CHILD", "1")
        .env("MAESTRO_REMOVED_CWD_PARENT", directory.text())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    false
}
/// Enter then unlink a working directory without altering the parent test process.
fn remove_cwd() -> Directory {
    let directory = Directory::new();
    let removed = directory.path.join("removed");
    fs::create_dir(&removed).unwrap();
    std::env::set_current_dir(&removed).unwrap();
    fs::remove_dir(&removed).unwrap();
    directory
}

#[cfg(unix)]
#[test]
fn absolute_open_avoids_unneeded_cwd_reads() {
    if !removed_cwd_child("absolute_open_avoids_unneeded_cwd_reads") {
        return;
    }
    let directory = remove_cwd();
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
    let directory = remove_cwd();
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
struct Directory {
    /// Authored absolute native location.
    path: PathBuf,
}
impl Directory {
    /// Create a fresh directory without environment mutation.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("maestro-session-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self { path }
    }
    /// Borrow its native spelling.
    fn text(&self) -> &str {
        self.path.to_str().unwrap()
    }
    /// Write fixture bytes to one selected file.
    fn file(&self, name: &str, bytes: impl AsRef<[u8]>) -> String {
        let path = self.path.join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).unwrap();
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
    let fifo = directory.path.join("stat-error.jsonl");
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
