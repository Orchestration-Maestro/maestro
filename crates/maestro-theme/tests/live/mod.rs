//! Live-theme scratch directories, state and shared fixture access.
mod document;

pub use crate::support::environment::{TRUECOLOR, in_child, per_environment};
pub use document::custom_json;
use maestro_theme::{ThemeDirectories, ThemeState};
use std::fs;
use std::rc::Rc;

/// Directory of the shipped theme documents.
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme");
/// A fresh directory, removed on drop.
pub struct Scratch(maestro_test_directory::Scratch);

impl Scratch {
    /// Create the directory.
    pub fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }

    /// Path of an entry below the scratch directory.
    pub fn path(&self, relative: &str) -> String {
        // Join by component, so a Windows path keeps one separator throughout.
        relative
            .split('/')
            .fold(self.0.path().to_path_buf(), |path, part| path.join(part))
            .to_string_lossy()
            .into_owned()
    }

    /// Write a file below the scratch directory.
    pub fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// Whether this host stores the supplied leaf names as distinct entries.
    pub fn keeps_distinct(&self, names: &[&std::ffi::OsStr]) -> bool {
        self.0.keeps_distinct(names).unwrap()
    }
}

/// A state over shipped themes and the scratch's `custom` directory, created if needed
/// without clearing existing contents.
pub fn state(scratch: &Scratch) -> Rc<ThemeState> {
    fs::create_dir_all(scratch.path("custom")).unwrap();
    Rc::new(ThemeState::new(ThemeDirectories {
        themes_dir: SHIPPED.to_owned(),
        custom_themes_dir: scratch.path("custom"),
    }))
}
