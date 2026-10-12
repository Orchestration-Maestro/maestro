//! Disposable native histories, including one whose next publication really fails.
use maestro_session::SessionManager;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::json;
use std::path::PathBuf;

/// Scratch directory removed on drop.
pub struct Scratch(PathBuf);
impl Scratch {
    /// Create an empty owned directory.
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("maestro-tree-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    /// Directory spelling.
    pub fn text(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A published history whose session file was replaced by a directory, so every
/// further append fails natively; returns the manager and its first entry identity.
pub fn broken_history(scratch: &Scratch) -> (SessionManager, String) {
    let mut manager = SessionManager::create("/work", Some(scratch.text()), "unused").unwrap();
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
