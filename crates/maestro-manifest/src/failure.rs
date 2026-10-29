//! Load failures: where the selection is written, what it names, what is wrong and how to fix it.

use std::fmt;

/// A refusal to load the manifest.
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    /// The settings file that selects the manifest.
    pub file: String,
    /// The selected source, or the value when it is not a source.
    pub source: String,
    /// What is wrong.
    pub problem: String,
    /// What to change.
    pub fix: String,
}

impl fmt::Display for Failure {
    /// Writes `<file>: <source>: <problem>. Fix: <fix>`.
    fn fmt(&self, _formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

/// The problem for a release that has no manifest root.
#[must_use]
pub fn no_manifest_root(_source: &str) -> String {
    String::new()
}
