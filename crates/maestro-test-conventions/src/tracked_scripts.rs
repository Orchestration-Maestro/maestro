//! Rejects tracked script files: the repository is Rust only.

use std::path::Path;
use std::process::Command;

/// Extensions of the script languages the repository does not track.
const EXTENSIONS: [&str; 9] = ["js", "mjs", "cjs", "jsx", "ts", "tsx", "py", "sh", "ps1"];

/// Tracked scripts that leave with the ticket that moves them.
const PENDING: [&str; 1] = ["examples/sdk/rpc.py"];

/// Fail when the index tracks a script file.
///
/// Browser loaders come out of the build and live under `target/`; untracked files are
/// not checked. A root without a `.git` entry has no index, so there is nothing to check.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    if !root.join(".git").exists() {
        return Ok(());
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
    for path in listing.split('\0').filter(|path| !path.is_empty()) {
        if banned(path) && !PENDING.contains(&path) {
            return Err(format!(
                "{path}: the repository tracks no script files; write the logic in Rust and keep build output under target/"
            ));
        }
    }
    Ok(())
}

/// Whether the path names a script file.
fn banned(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|banned| banned.eq_ignore_ascii_case(extension))
        })
}
