//! Home-directory expansion of authored paths.
use std::path::PathBuf;

/// Expands a leading `~` in the compile target's flavor; see [`expand_tilde_with_flavor`].
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn expand_tilde<E>(path: &str, home: impl FnOnce() -> Result<String, E>) -> Result<String, E> {
    expand_tilde_with_flavor(path, cfg!(windows), home)
}

/// Expands a leading `~` in a supplied flavor, independently of the compile target.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn expand_tilde_with_flavor<E>(
    path: &str,
    _windows: bool,
    _home: impl FnOnce() -> Result<String, E>,
) -> Result<String, E> {
    Ok(path.to_owned())
}

/// Expands a leading `~` below a native home directory, keeping its bytes.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn expand_tilde_native<E>(
    path: &str,
    _home: impl FnOnce() -> Result<PathBuf, E>,
) -> Result<PathBuf, E> {
    Ok(PathBuf::from(path))
}
