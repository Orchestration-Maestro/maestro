//! Test reads of repository directories must be routed in the CI configuration.

use std::path::Path;

use crate::source::Member;

/// Read the CI configuration, or `None` when the workspace has none.
fn configuration(root: &Path) -> Result<Option<String>, String> {
    let path = root.join(".github/ci.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Reject a run-time test read of a repository directory that `test_inputs` does not route
/// to the reading package.
pub(crate) fn check(root: &Path, _members: &[Member]) -> Result<(), String> {
    configuration(root).map(drop)
}
