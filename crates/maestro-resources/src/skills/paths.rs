//! Caller-specific path observations.
use crate::ResourceOperations;
/// Resolves aliases, preserving the input spelling on failure.
pub fn canonicalize_path(path: &str, operations: &dyn ResourceOperations) -> String {
    operations.realpath(path).unwrap_or_else(|_| path.into())
}
/// Tests the six nonlocal source prefixes after trimming.
pub fn is_local_path(value: &str) -> bool {
    let value = trim(value);
    !["npm:", "git:", "github:", "https:", "http:", "ssh:"]
        .iter()
        .any(|p| value.starts_with(p))
}
pub(super) fn trim(value: &str) -> &str {
    value.trim()
}
pub(super) fn join(parts: &[&str]) -> String {
    let mut path = std::path::PathBuf::new();
    for part in parts {
        path.push(part)
    }
    path.to_string_lossy().into_owned()
}
pub(super) fn dirname(value: &str) -> String {
    std::path::Path::new(value)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".".into())
}
pub(super) fn basename(value: &str) -> String {
    std::path::Path::new(value)
        .file_name()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
pub(super) fn resolve(
    parts: &[&str],
    operations: &dyn ResourceOperations,
) -> Result<String, crate::ResourceError> {
    let path = join(parts);
    let path = if std::path::Path::new(&path).is_absolute() {
        path
    } else {
        join(&[&operations.current_dir()?, &path])
    };
    std::path::absolute(path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| crate::ResourceError {
            message: Some(e.to_string()),
        })
}
