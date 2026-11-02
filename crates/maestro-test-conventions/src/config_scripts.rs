//! Rejects shell scripts in the runner configuration directory.

use std::path::Path;

/// Fail when a shell script sits directly under `.config/`, where nextest's setup entries
/// would run it; the development tool owns that logic. A workspace without the directory
/// has nothing to check.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let directory = root.join(".config");
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    for entry in entries {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        if path.extension().is_some_and(|extension| extension == "sh") {
            return Err(format!(
                "{}: setup logic belongs in Rust in the development tool (maestro-tooling), not in a shell script",
                path.display()
            ));
        }
    }
    Ok(())
}
