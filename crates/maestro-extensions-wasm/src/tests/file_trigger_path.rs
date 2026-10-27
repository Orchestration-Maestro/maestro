//! Exclusive ownership of the authored trigger path for serialized Linux tests.
#![cfg(target_os = "linux")]
/// Exclusively owns the authored trigger path, never an existing user's file.
pub struct Trigger(std::fs::File);
impl Trigger {
    /// Refuses to touch an existing trigger; serial nextest admission prevents test collisions.
    pub fn new() -> Self {
        Self(std::fs::File::create_new("/tmp/agent-trigger.txt").expect(
            "file-trigger test requires exclusive /tmp/agent-trigger.txt; existing file left untouched",
        ))
    }

    /// Changes only the exclusively created file's contents.
    pub fn write(&mut self, text: &str) {
        use std::io::{Seek, Write};
        self.0.set_len(0).unwrap();
        self.0.rewind().unwrap();
        self.0.write_all(text.as_bytes()).unwrap();
        self.0.flush().unwrap();
    }
}
impl Drop for Trigger {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Ok(current) = std::fs::metadata("/tmp/agent-trigger.txt") {
            let owned = self.0.metadata().unwrap();
            if (current.dev(), current.ino()) == (owned.dev(), owned.ino()) {
                let _ = std::fs::remove_file("/tmp/agent-trigger.txt");
            }
        }
    }
}
