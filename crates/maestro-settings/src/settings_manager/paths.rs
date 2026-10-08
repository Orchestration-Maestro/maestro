//! Lexical path joining.

use std::ffi::{OsStr, OsString};
use std::path::{Component, MAIN_SEPARATOR_STR, Path, PathBuf, is_separator};

/// Joins `parts` with the platform's separator as text, skipping empty ones, then
/// folds the result once without reading the file system. Only the first component
/// can be a prefix. A `..` removes the segment before it; with none, it is dropped
/// below a root and kept in front of a path that has no root, drive-relative ones
/// included. A trailing separator is kept and an empty result is `.`.
pub(super) fn join(parts: &[&OsStr]) -> PathBuf {
    let mut text = OsString::new();
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if !text.is_empty() {
            text.push(MAIN_SEPARATOR_STR);
        }
        text.push(part);
    }
    let path = Path::new(&text);
    let rooted = path.has_root();
    let (mut folded, mut names) = (OsString::new(), Vec::new());
    for part in path.components() {
        match part {
            Component::Prefix(prefix) => folded.push(prefix.as_os_str()),
            Component::RootDir => folded.push(MAIN_SEPARATOR_STR),
            Component::CurDir => {}
            Component::ParentDir if matches!(names.last(), Some(Component::Normal(_))) => {
                names.pop();
            }
            Component::ParentDir if rooted => {}
            Component::ParentDir | Component::Normal(_) => names.push(part),
        }
    }
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            folded.push(MAIN_SEPARATOR_STR);
        }
        folded.push(name.as_os_str());
    }
    if names.is_empty() && !rooted {
        folded.push(".");
    }
    let trailing = text
        .as_encoded_bytes()
        .last()
        .is_some_and(|byte| is_separator(char::from(*byte)));
    if trailing && !(names.is_empty() && rooted) {
        folded.push(MAIN_SEPARATOR_STR);
    }
    folded.into()
}
