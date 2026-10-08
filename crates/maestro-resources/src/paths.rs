//! Resource path classification and canonicalization fallback.

use crate::{frontmatter::text_whitespace, skills::ResourceOperations};
use std::path::{Path, PathBuf};

/// Classify local paths, excluding the known lowercase package and URL prefixes.
pub fn is_local_path(value: &str) -> bool {
    let value = value.trim_matches(text_whitespace);
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

/// Return a canonical path or retain the original spelling on failure.
pub fn canonicalize_path(path: &Path, operations: &dyn ResourceOperations) -> PathBuf {
    operations
        .canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Resolve relative or home-expanded inputs while retaining authored absolute spelling.
pub(crate) fn resolve_skill_path(
    input: &std::path::Path,
    cwd: &std::path::Path,
    home: &std::path::Path,
) -> std::path::PathBuf {
    let text = input.to_string_lossy();
    let text = text.trim_matches(crate::frontmatter::text_whitespace);
    if let Some(suffix) = text.strip_prefix('~') {
        return join_path(&[home, Path::new(suffix)]);
    }
    let path = std::path::Path::new(text);
    if path.is_absolute() {
        path.into()
    } else {
        join_path(&[cwd, path])
    }
}
/// Concatenate authored parts before lexical normalization; only the initial prefix
/// roots the result. Parents pop normal components, disappear at a root, or remain
/// in relative paths. Later rooted parts never replace earlier parts.
pub(crate) fn join_path(parts: &[&Path]) -> PathBuf {
    let mut joined = std::ffi::OsString::new();
    for part in parts.iter().filter(|part| !part.as_os_str().is_empty()) {
        if !joined.is_empty() {
            joined.push(std::path::MAIN_SEPARATOR_STR);
        }
        joined.push(part.as_os_str());
    }
    normalize_path(Path::new(&joined))
}
/// Normalize lexical dot components without consulting filesystem targets.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut result = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if result.file_name().is_some_and(|name| name != "..") => {
                result.pop();
            }
            Component::ParentDir if result.has_root() => {}
            _ => result.push(component.as_os_str()),
        }
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}
