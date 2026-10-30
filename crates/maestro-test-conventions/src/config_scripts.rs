//! Rejects shell scripts in the runner configuration directory.

use std::path::Path;

/// The extensions of the shell scripts the runner configuration may not hold.
const SHELL_EXTENSIONS: [&str; 2] = ["sh", "bash"];

/// Fail when a shell script sits anywhere under `.config/`, where nextest's setup entries
/// would run it; the development tool owns that logic. A workspace without the directory
/// has nothing to check.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    scan(&root.join(".config"))
}

/// Fail on a shell script in `directory` or any directory below it. A directory that does
/// not exist has nothing to check.
fn scan(directory: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    for entry in entries {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        if path.is_dir() {
            scan(&path)?;
        } else if path
            .extension()
            .is_some_and(|extension| SHELL_EXTENSIONS.iter().any(|shell| extension == *shell))
        {
            return Err(format!(
                "{}: setup logic belongs in Rust in the development tool (maestro-tooling), not in a shell script",
                path.display()
            ));
        }
    }
    Ok(())
}
