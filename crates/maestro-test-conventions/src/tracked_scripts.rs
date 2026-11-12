//! Rejects tracked script files: the repository is Rust only.

use crate::tracked_files;
use std::path::Path;

/// Extensions of the script languages the repository does not track.
const EXTENSIONS: [&str; 9] = ["js", "mjs", "cjs", "jsx", "ts", "tsx", "py", "sh", "ps1"];

/// Fail when the index tracks a script file.
///
/// Browser loaders come out of the build and live under `target/`; untracked files are
/// not checked. A root without a `.git` entry has no index, so there is nothing to check.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    for path in tracked_files::paths(root)? {
        if banned(&path) {
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
