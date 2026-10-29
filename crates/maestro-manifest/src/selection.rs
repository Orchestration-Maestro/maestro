//! The `manifest` setting: which release a settings document names.

use serde_json::Value;

/// Reads the manifest source a raw settings document names.
///
/// An absent key, an empty string or an object with an empty `source` names nothing.
///
/// # Errors
/// Returns the problem with the value when it is neither a source string nor an object whose
/// `source` is a string and whose resource filters are lists of strings.
pub fn parse(_document: &Value) -> Result<Option<String>, String> {
    Err("not read".into())
}

/// Whether a project selection is set aside because the user's manifest locks `manifest`
/// and the project names another repository.
#[must_use]
pub fn project_ignored(_user: &str, _project: &str, _user_locks_manifest: bool) -> bool {
    false
}
