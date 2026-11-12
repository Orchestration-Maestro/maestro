//! Lists the files the repository index tracks.

use std::path::Path;
use std::process::Command;

/// The tracked paths of `root`, relative to it; empty when `root` has no `.git` entry.
pub(crate) fn paths(root: &Path) -> Result<Vec<String>, String> {
    if !root.join(".git").exists() {
        return Ok(Vec::new());
    }
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("git ls-files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let listing =
        String::from_utf8(output.stdout).map_err(|error| format!("git ls-files: {error}"))?;
    Ok(listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}
