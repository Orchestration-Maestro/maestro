//! Native home-directory selection, its supplied-observation helper and its text form.

use std::{io, path::PathBuf};

/// The supplied `home` when present, otherwise what `account` gives.
///
/// A present value is returned untouched, an empty one included; `account` runs
/// only when `home` is absent and its result, absent or not, is returned. Reading
/// the environment variable and the account database stay with the caller.
///
/// # Examples
///
/// ```
/// assert_eq!(maestro_path::home_dir(Some(""), || Some("/account")), Some(""));
/// assert_eq!(maestro_path::home_dir(None, || Some("/account")), Some("/account"));
/// ```
pub fn home_dir<T>(home: Option<T>, account: impl FnOnce() -> Option<T>) -> Option<T> {
    home.or_else(account)
}

/// The native home directory, preserving a present Unix `HOME`, even when empty.
///
/// Otherwise reads the account home through the standard library.
#[must_use]
pub fn native_home_dir() -> Option<std::path::PathBuf> {
    home_dir(
        std::env::var_os("HOME")
            .filter(|_| cfg!(unix))
            .map(std::path::PathBuf::from),
        std::env::home_dir,
    )
}

/// The home directory as text.
///
/// # Errors
/// Returns a not-found error when the host knows no home directory, or an invalid-data one
/// when the home directory is not valid UTF-8.
pub fn home_directory() -> io::Result<String> {
    home_directory_text(native_home_dir())
}

/// Convert one observed native home into its text.
///
/// # Errors
/// Returns a not-found error for an unknown home, or an invalid-data one for bytes that are
/// not valid UTF-8.
pub fn home_directory_text(home: Option<PathBuf>) -> io::Result<String> {
    home.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))?
        .into_os_string()
        .into_string()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "home directory is not valid UTF-8",
            )
        })
}
