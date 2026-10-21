//! Shared fixtures for the live-theme tests: scratch directories, checked children with
//! their own environment and custom theme documents.
use maestro_theme::{ThemeDirectories, ThemeState};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;

/// Directory of the shipped theme documents.
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme");
/// The shipped dark theme document.
const DARK: &str = include_str!("../../assets/theme/dark.json");
/// A truecolor terminal, so theme loads without an explicit mode emit RGB.
pub const TRUECOLOR: &[(&str, &str)] = &[("COLORTERM", "truecolor")];

/// Run `test`'s cases in one child per distinct environment, each holding only those
/// variables. Returns the cases of this child's environment, or `None` in the calling
/// process once every child passed.
///
/// # Panics
/// Panics if a child fails or does not start exactly `test`.
pub fn per_environment<C>(
    test: &str,
    cases: impl IntoIterator<Item = C>,
    environment: impl Fn(&C) -> &[(&'static str, &'static str)],
) -> Option<Vec<C>> {
    maestro_test_process::run_per_environment(test, cases, |case| {
        let mut env: BTreeMap<String, String> = environment(case)
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        // The temporary root stays, so a child can still create scratch directories.
        for name in ["TMPDIR", "TMP", "TEMP"] {
            if let Ok(value) = std::env::var(name) {
                env.entry(name.to_owned()).or_insert(value);
            }
        }
        env
    })
    .unwrap()
}

/// Whether this process runs `test`'s body: true in its child, whose environment is
/// exactly `env`; false in the calling process once that child passed.
pub fn in_child(test: &str, env: &'static [(&'static str, &'static str)]) -> bool {
    per_environment(test, [env], |env| env).is_some()
}

/// A fresh directory, removed on drop.
pub struct Scratch(maestro_test_directory::Scratch);

impl Scratch {
    /// Create the directory.
    pub fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }

    /// Path of an entry below the scratch directory.
    pub fn path(&self, relative: &str) -> String {
        self.0.path().join(relative).to_string_lossy().into_owned()
    }

    /// Write a file below the scratch directory.
    pub fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

/// A state over the shipped themes and the scratch's `custom` directory, created empty.
pub fn state(scratch: &Scratch) -> Rc<ThemeState> {
    fs::create_dir_all(scratch.path("custom")).unwrap();
    Rc::new(ThemeState::new(ThemeDirectories {
        themes_dir: SHIPPED.to_owned(),
        custom_themes_dir: scratch.path("custom"),
    }))
}

/// The shipped dark document under another name with a literal accent.
pub fn custom_json(name: &str, accent: &str) -> Value {
    let mut json: Value = serde_json::from_str(DARK).unwrap();
    json["name"] = name.into();
    json["colors"]["accent"] = accent.into();
    json
}
