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
        return normalize_path(&home.join(suffix.strip_prefix('/').unwrap_or(suffix)));
    }
    let path = std::path::Path::new(text);
    if path.is_absolute() {
        path.into()
    } else {
        normalize_path(&cwd.join(path))
    }
}
/// Normalize lexical dot components without consulting filesystem targets.
pub(crate) fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
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
    result
}
