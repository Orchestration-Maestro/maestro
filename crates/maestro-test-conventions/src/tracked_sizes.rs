//! Rejects tracked files above a size limit.

use crate::tracked_files;
use std::path::Path;

/// The largest tracked file, in bytes. The largest fixture today is 8.3 MB.
const LIMIT: u64 = 10 * 1024 * 1024;

/// Fail when a tracked file is larger than the limit.
///
/// A fixture that repeats its data grows a file past the limit; store the data once
/// and expand it at run time instead. Untracked files and a root without an index
/// are not checked.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    for path in tracked_files::paths(root)? {
        let length = match std::fs::symlink_metadata(root.join(&path)) {
            Ok(metadata) => metadata.len(),
            // A tracked file removed from the working tree has no size to check.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{path}: {error}")),
        };
        if length > LIMIT {
            return Err(format!(
                "{path}: {length} bytes exceeds the {LIMIT}-byte limit on tracked files; store repeated data once and expand it when the test runs"
            ));
        }
    }
    Ok(())
}
