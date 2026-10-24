//! Disposable native histories, including one whose next publication really fails.
use maestro_session::SessionManager;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_test_directory::{Scratch, temporary_root};
use serde_json::json;

/// `name` joined below `root` as the stored string spelling.
pub fn below(root: &std::path::Path, name: &str) -> String {
    root.join(name).to_str().unwrap().to_owned()
}

/// A disposable root spelled as the temporary directory spells it, not canonicalized,
/// so a fixture can supply a spelling whose dot segments survive joining.
pub fn uncanonicalized_scratch() -> Scratch {
    Scratch::new_in(&temporary_root()).unwrap()
}

/// `name` joined below `root` as the stored string spelling, asserting that the joined
/// spelling still holds the dot segments the caller supplied; on Windows, joining below a
/// verbatim root removes them, which would leave a preservation assertion nothing to tell apart.
pub fn below_keeping_dots(root: &std::path::Path, name: &str) -> String {
    let spelling = below(root, name);
    assert!(
        spelling.contains(name),
        "the joined spelling keeps its dot segments: {spelling}"
    );
    spelling
}

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
