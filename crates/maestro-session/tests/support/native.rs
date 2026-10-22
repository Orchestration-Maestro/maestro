//! Disposable native histories, including one whose next publication really fails.
use maestro_session::SessionManager;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_test_directory::Scratch;
use serde_json::json;

/// A published history whose session file was replaced by a directory, so every
/// further append fails natively; returns the manager and its first entry identity.
pub fn broken_history(scratch: &Scratch) -> (SessionManager, String) {
    let mut manager =
        SessionManager::create("/work", Some(scratch.path().to_str().unwrap()), "unused").unwrap();
    let first = manager
        .append_message(json!({"role": "user", "content": "first", "timestamp": 1}))
        .unwrap();
    manager
        .append_message(json!({"role": "assistant", "content": "second", "timestamp": 2}))
        .unwrap();
    let file = manager.get_session_file().unwrap().to_owned();
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    (manager, first)
}
