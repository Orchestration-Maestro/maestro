//! Lexical path normalization.

use std::path::{Component, Path, PathBuf};

/// Folds `.` and `..` segments of `path` without reading the file system: a `..`
/// removes the segment before it, is dropped at a root and is kept in front of a
/// relative path.
pub(super) fn normalized(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => parts.push(part),
            },
            _ => parts.push(part),
        }
    }
    parts.into_iter().collect()
}
