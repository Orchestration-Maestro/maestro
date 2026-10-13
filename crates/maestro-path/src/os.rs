//! Selection of a home directory from caller-supplied observations.

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
