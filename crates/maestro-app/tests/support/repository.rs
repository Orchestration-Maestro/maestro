//! Real isolated metadata files for footer behavior witnesses.
use super::Entry;
use std::collections::BTreeMap;

pub struct Repository(maestro_test_directory::Scratch);
impl Default for Repository {
    fn default() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }
}
impl Repository {
    pub fn new(files: BTreeMap<String, Entry>) -> Self {
        let repo = Self::default();
        for (path, entry) in files {
            repo.set(&path, entry);
        }
        repo
    }
    pub fn path(&self, name: &str) -> String {
        // Join by component, so a Windows path keeps one separator throughout.
        name.trim_start_matches('/')
            .split('/')
            .fold(self.0.path().to_path_buf(), |path, part| path.join(part))
            .to_string_lossy()
            .into_owned()
    }
    pub fn directory(&self, name: &str) {
        std::fs::create_dir_all(self.path(name)).unwrap();
    }
    pub fn remove(&self, name: &str) {
        let path = self.path(name);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            std::fs::remove_dir_all(path).unwrap();
        } else if std::fs::symlink_metadata(&path).is_ok() {
            std::fs::remove_file(path).unwrap();
        }
    }
    pub fn text(&self, name: &str, text: &str) {
        let path = self.path(name);
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        let trimmed = text.trim_matches(maestro_text::is_whitespace_scalar);
        let target = trimmed
            .strip_prefix("gitdir:")
            .unwrap_or(trimmed)
            .trim_matches(maestro_text::is_whitespace_scalar);
        let text = if target.starts_with('/')
            && (name.ends_with("/.git") || name.ends_with("/commondir"))
        {
            text.replacen(target, &self.path(target), 1)
        } else {
            text.to_owned()
        };
        std::fs::write(path, text).unwrap();
    }
    pub fn set(&self, name: &str, entry: Entry) {
        self.remove(name);
        match entry {
            Entry::Text(text) => self.text(name, &text),
            Entry::Flag(true) => self.directory(name),
            Entry::Flag(false) => {
                let path = self.path(name);
                std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
                #[cfg(unix)]
                drop(std::os::unix::net::UnixListener::bind(path).unwrap());
                #[cfg(not(unix))]
                std::fs::write(path, "not a metadata marker").unwrap();
            }
        }
    }
    pub fn unreadable(&self, name: &str) {
        self.remove(name);
        self.directory(name);
    }
}
