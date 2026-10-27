//! Exclusive ownership of the authored trigger path for serialized Linux tests.
#![cfg(target_os = "linux")]
/// Exclusively owns the authored trigger path, never an existing user's file.
pub struct Trigger {
    /// The exclusively created file.
    file: std::fs::File,
    /// The host path owned by this fixture.
    path: std::path::PathBuf,
}
impl Trigger {
    /// Refuses to touch an existing trigger; serial nextest admission prevents test collisions.
    pub fn new_in(_root: &std::path::Path) -> Self {
        Self {
            file: std::fs::File::create_new("/tmp/agent-trigger.txt").expect(
                "file-trigger test requires exclusive /tmp/agent-trigger.txt; existing file left untouched",
            ),
            path: "/tmp/agent-trigger.txt".into(),
        }
    }

    /// The host path this fixture owns.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Changes only the exclusively created file's contents.
    pub fn write(&mut self, text: &str) {
        use std::io::{Seek, Write};
        self.file.set_len(0).unwrap();
        self.file.rewind().unwrap();
        self.file.write_all(text.as_bytes()).unwrap();
        self.file.flush().unwrap();
    }
}
impl Drop for Trigger {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Ok(current) = std::fs::metadata("/tmp/agent-trigger.txt") {
            let owned = self.file.metadata().unwrap();
            if (current.dev(), current.ino()) == (owned.dev(), owned.ino()) {
                let _ = std::fs::remove_file("/tmp/agent-trigger.txt");
            }
        }
    }
}
