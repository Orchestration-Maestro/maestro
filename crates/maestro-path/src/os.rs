//! Native home-directory selection and its supplied-observation helper.

/// The supplied `home` when present, otherwise what `account` gives.
///
/// A present value is returned untouched, an empty one included; `account` runs
/// only when `home` is absent and its result, absent or not, is returned. Reading
/// the environment variable and the account database stay with the caller.
///
/// # Examples
///
/// ```
/// assert_eq!(maestro_path::homedir(Some(""), || Some("/account")), Some(""));
/// assert_eq!(maestro_path::homedir(None, || Some("/account")), Some("/account"));
/// ```
pub fn homedir<T>(home: Option<T>, account: impl FnOnce() -> Option<T>) -> Option<T> {
    home.or_else(account)
}

/// The native home directory, preserving a present Unix `HOME`, even when empty.
///
/// Otherwise reads the account home through the standard library.
#[must_use]
pub fn native_homedir() -> Option<std::path::PathBuf> {
    homedir(
        std::env::var_os("HOME")
            .filter(|_| cfg!(unix))
            .map(std::path::PathBuf::from),
        std::env::home_dir,
    )
}
