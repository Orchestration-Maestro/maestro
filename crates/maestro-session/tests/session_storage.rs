//! Conditional publication through the native session boundary.
#![cfg(not(target_arch = "wasm32"))]
use maestro_session::session_manager::rewrite_session_file;
use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn conditional_rewrite_returns_concurrent_append_without_overwriting_it() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("session.jsonl");
    let original = b"{\"type\":\"session\",\"id\":\"kept\",\"version\":3}\n";
    let appended = [
        original.as_slice(),
        b"{\"type\":\"custom\",\"id\":\"later\"}\n",
    ]
    .concat();
    fs::write(&path, &appended).unwrap();
    let result = rewrite_session_file(
        path.to_str().unwrap(),
        &Sha256::digest(original).into(),
        b"replacement\n",
    )
    .unwrap();
    assert_eq!(result, Some(appended.clone()));
    assert_eq!(fs::read(&path).unwrap(), appended);
    assert_eq!(
        rewrite_session_file(
            path.to_str().unwrap(),
            &Sha256::digest(&appended).into(),
            original,
        )
        .unwrap(),
        None
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn conditional_rewrite_accepts_a_full_length_session_leaf() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join(format!("{}.jsonl", "x".repeat(249)));
    fs::write(&path, b"before").unwrap();
    assert_eq!(
        rewrite_session_file(
            path.to_str().unwrap(),
            &Sha256::digest(b"before").into(),
            b"after"
        )
        .unwrap(),
        None
    );
    assert_eq!(fs::read(&path).unwrap(), b"after");
}

#[cfg(unix)]
#[test]
fn append_to_existing_session_needs_no_directory_write_permission() {
    use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("session.jsonl");
    fs::write(
        &path,
        b"{\"type\":\"session\",\"id\":\"kept\",\"version\":3,\"cwd\":\"/work\"}\n",
    )
    .unwrap();
    let mut manager = SessionManager::open(path.to_str().unwrap(), None, Some("/work")).unwrap();
    let Some(_restore) = maestro_test_directory::deny(root.path(), 0o555).unwrap() else {
        return;
    };
    manager
        .append_message(serde_json::json!({"role":"assistant","content":"after"}))
        .unwrap();
    assert_eq!(manager.get_session_file(), Some(path.to_str().unwrap()));
    assert!(fs::read_to_string(&path).unwrap().contains("after"));
}

#[test]
fn writable_loaders_replay_interrupted_history_before_selecting_parent() {
    use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
    for loader in ["open", "switch", "fork"] {
        let root = maestro_test_directory::Scratch::new().unwrap();
        let path = root.path().join("session.jsonl");
        let prefix = b"{\"type\":\"session\",\"id\":\"kept\",\"version\":3,\"cwd\":\"/work\"}\n";
        let complete = [prefix.as_slice(), b"{\"type\":\"message\",\"id\":\"last\",\"parentId\":null,\"timestamp\":\"2026-01-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"content\":\"kept\"}}\n"].concat();
        fs::write(&path, prefix).unwrap();
        stage_interrupted_history(&path, &complete);
        let directory = root.path().to_str().unwrap();
        let path = path.to_str().unwrap();
        let mut manager = match loader {
            "open" => SessionManager::open(path, None, Some("/work")).unwrap(),
            "switch" => {
                let mut manager =
                    SessionManager::create("/work", Some(directory), "unused").unwrap();
                manager.set_session_file(path).unwrap();
                manager
            }
            _ => SessionManager::fork_from(path, "/work", Some(directory), "unused").unwrap(),
        };
        manager
            .append_message(serde_json::json!({"role":"user","content":"after"}))
            .unwrap();
        let entries = maestro_session::session_manager::load_entries_from_file(
            manager.get_session_file().unwrap(),
        )
        .unwrap();
        assert_eq!(entries.len(), 3, "{loader} lost recovered history");
        assert_eq!(
            entries[2]["parentId"], "last",
            "{loader} selected a truncated parent"
        );
        assert!(fs::read(path).unwrap().starts_with(&complete));
    }
}

/// Seeds the durable record left by an interrupted in-place publication.
fn stage_interrupted_history(path: &std::path::Path, bytes: &[u8]) -> std::path::PathBuf {
    use std::io::Write;
    let holder = maestro_lock::acquire_write(path, true).unwrap();
    let digest = Sha256::digest(holder.path().as_os_str().as_encoded_bytes());
    let mut name = String::new();
    for byte in digest {
        use std::fmt::Write;
        write!(name, "{byte:02x}").unwrap();
    }
    name.push_str(".recovery");
    let root = maestro_path::native_homedir()
        .unwrap()
        .join(".maestro/state/file-locks");
    let mut record = fs::File::create(root.join(&name)).unwrap();
    record.write_all(&digest).unwrap();
    record.write_all(&[0; 5]).unwrap();
    record.write_all(bytes).unwrap();
    record.sync_all().unwrap();
    let sync = fs::File::open(&root).and_then(|file| file.sync_all());
    #[cfg(windows)]
    let sync = sync.or_else(|error| match error.kind() {
        std::io::ErrorKind::Unsupported
        | std::io::ErrorKind::InvalidInput
        | std::io::ErrorKind::PermissionDenied => Ok(()),
        _ => Err(error),
    });
    sync.unwrap();
    root.join(name)
}

#[test]
fn opening_missing_nested_history_retains_unpublished_session() {
    use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("new").join("nested").join("session.jsonl");
    let mut manager = SessionManager::open(path.to_str().unwrap(), None, Some("/work")).unwrap();
    assert!(path.parent().unwrap().is_dir());
    assert!(!path.exists());
    manager
        .append_message(serde_json::json!({"role":"assistant","content":"after"}))
        .unwrap();
    assert!(path.exists());
    assert_eq!(manager.get_session_file(), Some(path.to_str().unwrap()));
}

#[test]
fn nonpersisting_selection_leaves_target_and_recovery_unchanged() {
    use maestro_session::SessionManager;
    let root = maestro_test_directory::Scratch::new().unwrap();
    let path = root.path().join("session.jsonl");
    let prefix = b"{\"type\":\"session\",\"id\":\"kept\",\"version\":3,\"cwd\":\"/work\"}\n";
    fs::write(&path, prefix).unwrap();
    let complete = [
        prefix.as_slice(),
        b"{\"type\":\"custom\",\"id\":\"later\"}\n",
    ]
    .concat();
    let recovery = stage_interrupted_history(&path, &complete);
    let record = fs::read(&recovery).unwrap();
    let mut manager = SessionManager::in_memory(Some("/work")).unwrap();
    manager.set_session_file(path.to_str().unwrap()).unwrap();
    assert!(!manager.is_persisted());
    assert_eq!(fs::read(&path).unwrap(), prefix);
    assert_eq!(fs::read(&recovery).unwrap(), record);
    drop(maestro_lock::acquire_write(&path, true).unwrap());
}
