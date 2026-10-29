//! Messages shown to the user once a load has finished.

/// The notice for a checkout that was restored to its pin, with the files it reset or removed.
#[must_use]
pub fn restored(_source: &str, _files: &[String]) -> String {
    String::new()
}

/// The warning for a checkout that drifted and could not be restored.
#[must_use]
pub fn drifted(_source: &str, _files: &[String], _error: &str) -> String {
    String::new()
}

/// The warning for a project selection that the user's manifest overrules.
#[must_use]
pub fn ignored(_project: &str, _user: &str) -> String {
    String::new()
}
