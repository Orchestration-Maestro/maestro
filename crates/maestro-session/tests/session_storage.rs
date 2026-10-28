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
