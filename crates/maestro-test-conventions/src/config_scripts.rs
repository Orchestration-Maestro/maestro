//! Rejects shell scripts in the runner configuration directory.

use std::path::Path;

/// Fail when a shell script sits anywhere under `.config/`, where nextest's setup entries
/// would run it; the development tool owns that logic. A workspace without the directory
/// has nothing to check.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    scan(&root.join(".config"))
}

/// Check `directory` and every directory below it.
fn scan(directory: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir() {
            scan(&path)?;
        } else if path.extension().is_some_and(|extension| extension == "sh") {
            return Err(format!(
                "{}: setup logic belongs in Rust in the development tool (maestro-tooling), not in a shell script",
                path.display()
            ));
        }
    }
    Ok(())
}
