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
